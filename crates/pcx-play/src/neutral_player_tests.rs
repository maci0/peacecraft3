use crate::scripts::{AssetIdentity, ScriptAsset};
use crate::{
    map_admission::{
        IdentifiedScript, MapAdmissionBudget, MapAdmissionInputs, ParsedMapContext,
        prepare_map_context,
    },
    map_scripts_import::MapScriptBudget,
    neutral_player::*,
};
use pcx_script::{Host, Value};
use std::{io::Cursor, sync::Arc};

#[path = "neutral_player_test_fixtures.rs"]
mod fixtures;
fn identity(source: &str) -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "test-v1".into(),
        source: source.into(),
    }
}
struct EmptyCatalog;
impl pcx_sim::world::ObjectCatalog for EmptyCatalog {
    fn resolve(
        &self,
        id: [u8; 4],
    ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
        Err(pcx_sim::world::ResolutionProblem::MissingRawcode(
            String::from_utf8_lossy(&id).into_owned(),
        ))
    }
}
fn context(word: u32, common: &str, map_source: &str, label: &str) -> Arc<ParsedMapContext> {
    context_with_wts(word, common, map_source, label, None)
}
fn context_with_wts(
    word: u32,
    common: &str,
    map_source: &str,
    label: &str,
    wts: Option<&[u8]>,
) -> Arc<ParsedMapContext> {
    let mut info = fixtures::info().canonical_bytes();
    info[8..12].copy_from_slice(&word.to_le_bytes());
    let mut files = vec![
        ("war3map.w3e", fixtures::terrain_bytes(2, 1)),
        ("war3map.wpm", fixtures::pathing_bytes(4, 2)),
        ("war3map.w3i", info),
        (
            "war3mapUnits.doo",
            pcx_map::Units {
                version: 8,
                subversion: 9,
                instances: vec![],
            }
            .canonical_bytes(),
        ),
        ("war3map.j", map_source.as_bytes().to_vec()),
    ];
    if let Some(wts) = wts {
        files.push(("war3map.wts", wts.to_vec()));
    }
    let raw = fixtures::build_synthetic_map(
        label,
        &files
            .iter()
            .map(|(n, b)| (*n, b.as_slice()))
            .collect::<Vec<_>>(),
    );
    let map = pcx_map::Map::parse(&raw).unwrap();
    let h = pcx_mpq::Header::parse(&mut Cursor::new(map.archive())).unwrap();
    let largest = files.iter().map(|(_, b)| b.len()).max().unwrap();
    let bj = "// deliberately nonempty synthetic prelude\n";
    let prelude = common.len() + bj.len();
    let scripts = MapScriptBudget {
        max_script_bytes: map_source.len(),
        max_wts_bytes: wts.map_or(0, <[u8]>::len),
        max_archive_bytes: map.archive().len(),
        max_archive_table_bytes: (h.hash_table_bytes() + h.block_table_bytes()) as usize,
        max_raw_packed_bytes: largest,
        max_decoded_bytes: largest,
        max_prelude_bytes: prelude,
        max_aggregate_input_bytes: map.archive().len()
            + prelude
            + map_source.len()
            + wts.map_or(0, <[u8]>::len),
    };
    let budget = MapAdmissionBudget {
        scripts,
        terrain_bytes: files[0].1.len(),
        pathing_bytes: files[1].1.len(),
        info_bytes: files[2].1.len(),
        units_bytes: files[3].1.len(),
        aggregate_input_bytes: map.archive().len()
            + prelude
            + files.iter().map(|(_, b)| b.len()).sum::<usize>(),
        required_fields: 0,
    };
    Arc::new(
        prepare_map_context(
            MapAdmissionInputs {
                map: &map,
                map_label: label,
                map_identity: &identity(label),
                common: IdentifiedScript {
                    identity: &identity("selected-common"),
                    asset: ScriptAsset {
                        label: "synthetic/common.j",
                        bytes: common.as_bytes(),
                    },
                },
                blizzard: IdentifiedScript {
                    identity: &identity("selected-bj"),
                    asset: ScriptAsset {
                        label: "synthetic/BJ.j",
                        bytes: bj.as_bytes(),
                    },
                },
                catalog_identity: &identity("selected-catalog"),
                catalog: &EmptyCatalog,
                required_fields: &[],
            },
            budget,
        )
        .unwrap(),
    )
}
const COMMON: &str = "native GetPlayerNeutralPassive takes nothing returns integer\nnative Other takes nothing returns integer\n";
const MAP: &str = "globals\ninteger result = GetPlayerNeutralPassive()\nendglobals\nfunction Read takes nothing returns integer\nreturn result\nendfunction\n";
#[derive(Default)]
struct Delegate {
    calls: usize,
}
impl Host for Delegate {
    fn native(&mut self, name: &str, _args: &[Value]) -> Result<Value, String> {
        self.calls += 1;
        if name == "Other" {
            Ok(Value::Int(313))
        } else {
            Err(format!("delegate refusal {name}"))
        }
    }
    fn table_string(&self, id: u32) -> Option<String> {
        Some(format!("table {id}"))
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        if id == 9 {
            Err("table read failure".into())
        } else {
            Ok(self.table_string(id))
        }
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        Some(format!("tag {tag}"))
    }
}
fn getter(threshold: u32, mask: i8, add: i8) -> Vec<u8> {
    let mut v = vec![0xa1];
    v.extend(0x11223344u32.to_le_bytes());
    v.extend([0x8b, 0x48, 0x28, 0x8d, 0x49, 0x18, 0xe8]);
    v.extend(37i32.to_le_bytes());
    v.extend([0x50, 0xe8]);
    v.extend((-73i32).to_le_bytes());
    v.extend([0x83, 0xc4, 4, 0x3d]);
    v.extend(threshold.to_le_bytes());
    v.extend([
        0x1b, 0xc0, 0x83, 0xe0, mask as u8, 0x83, 0xc0, add as u8, 0xc3,
    ]);
    v
}
fn header() -> Vec<u8> {
    let mut v = vec![0x83, 0xf8, 100, 0x76, 7, 0x33, 0xc0, 0xe9];
    v.extend(81i32.to_le_bytes());
    v.extend([0x83, 0xf8, 1, 0x72, 19, 0x53, 0x8b, 0xce, 0xe8]);
    v.extend((1234i32 - 25).to_le_bytes());
    v.extend([0x8d, 0x43, 4, 0x8b, 0xce, 0x50, 0xe8]);
    v.extend((1234i32 - 36).to_le_bytes());
    v
}
struct Input {
    image: Vec<u8>,
    profile: Vec<u8>,
    approval: LocalProfileApproval,
}
impl Input {
    fn import(&self) -> Result<VerifiedNeutralRule, NeutralError> {
        VerifiedNeutralRule::import(
            &self.image,
            &self.profile,
            &self.approval,
            RuleInputBudget {
                image_bytes: self.image.len(),
                profile_bytes: self.profile.len(),
                sections: 2,
            },
        )
    }
}
fn input(threshold: u32) -> Input {
    input_named(threshold, NATIVE_NAME)
}
fn input_named(threshold: u32, native_name: &str) -> Input {
    input_named_operands(threshold, native_name, -20, 61)
}
fn input_named_operands(threshold: u32, native_name: &str, mask: i8, add: i8) -> Input {
    let base = 0x0030_0000u32;
    let rva = 0x1000u32;
    let raw = 0x200usize;
    let mut image = vec![0; raw + 0x1000];
    image[..2].copy_from_slice(b"MZ");
    image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    image[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
    image[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    image[0x94..0x96].copy_from_slice(&0x60u16.to_le_bytes());
    image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    image[0xb4..0xb8].copy_from_slice(&base.to_le_bytes());
    image[0xd0..0xd4].copy_from_slice(&0x3000u32.to_le_bytes());
    let section = 0xf8;
    image[section + 8..section + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    image[section + 12..section + 16].copy_from_slice(&rva.to_le_bytes());
    image[section + 16..section + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    image[section + 20..section + 24].copy_from_slice(&(raw as u32).to_le_bytes());
    image[section + 36..section + 40].copy_from_slice(&0x6000_0060u32.to_le_bytes());
    let mut name = native_name.as_bytes().to_vec();
    name.push(0);
    let signature = b"()I\0".to_vec();
    let g = getter(threshold, mask, add);
    let h = header();
    let offsets = [0x30usize, 0x100, 0x140, 0x200, 0x300];
    let va = |offset| base + rva + offset as u32;
    let mut reg = vec![0x68];
    reg.extend(va(offsets[1]).to_le_bytes());
    reg.push(0xb9);
    reg.extend(va(offsets[2]).to_le_bytes());
    reg.push(0xba);
    reg.extend(va(offsets[3]).to_le_bytes());
    let parts = [reg, name, signature, g, h];
    for (at, part) in offsets.iter().zip(&parts) {
        image[raw + at..raw + at + part.len()].copy_from_slice(part);
    }
    let mut profile = b"PCXNPR\0\x01".to_vec();
    profile.extend((image.len() as u64).to_le_bytes());
    profile.extend(sha256(&image));
    for s in [
        "synthetic edition",
        "synthetic PE revision",
        "explicit local synthetic profile",
    ] {
        profile.extend((s.len() as u32).to_le_bytes());
        profile.extend(s.as_bytes());
    }
    for (at, part) in offsets.iter().zip(&parts) {
        profile.extend(va(*at).to_le_bytes());
        profile.extend((part.len() as u32).to_le_bytes());
        profile.extend(sha256(part));
    }
    for offset in [1u32, 6, 11] {
        profile.extend(offset.to_le_bytes());
    }
    let approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&profile));
    Input {
        image,
        profile,
        approval,
    }
}
fn rules() -> Arc<VerifiedNeutralRule> {
    Arc::new(input(257).import().unwrap())
}
#[test]
fn actual_admission_host_services_global_initializer_and_contexts_stay_separate() {
    for (word, expected, label) in [
        (256, 41, "selected-A"),
        (257, 61, "selected-B"),
        (u32::MAX, 61, "selected-C"),
    ] {
        let ctx = context(word, COMMON, MAP, label);
        let scripts = Arc::clone(&ctx.scripts);
        let host = SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()).unwrap();
        let digest = host.info_sha256();
        assert_eq!(digest, sha256(host.context().world.info.raw()));
        let mut vm = scripts.owned_vm(host);
        vm.init_globals().unwrap();
        assert_eq!(vm.global("result"), Value::Int(expected));
        assert_eq!(vm.call("Read", &[]).unwrap(), Value::Int(expected));
    }
}
#[test]
fn only_zeroarg_native_is_handled_and_other_host_behaviors_delegate() {
    let ctx = context(256, COMMON, MAP, "selected");
    let mut host = SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()).unwrap();
    assert!(
        host.native(NATIVE_NAME, &[Value::Int(1)])
            .unwrap_err()
            .contains("Arguments")
    );
    assert_eq!(host.native("Other", &[]).unwrap(), Value::Int(313));
    assert!(
        host.native("Missing", &[])
            .unwrap_err()
            .contains("delegate refusal")
    );
    assert_eq!(host.inner().calls, 2);
    assert_eq!(
        host.resolve_table_string(9),
        Err("table read failure".into())
    );
    assert_eq!(host.table_string(2), Some("table 2".into()));
    assert_eq!(host.code_tag("x"), Some("tag x".into()));
}
#[test]
fn wrong_or_missing_program_signature_refuses_before_globals() {
    for common in [
        "// no declaration\n",
        "native GetPlayerNeutralPassive takes nothing returns real\n",
        "native GetPlayerNeutralPassive takes integer x returns integer\n",
    ] {
        let ctx = context(
            256,
            common,
            "function Nothing takes nothing returns nothing\nendfunction\n",
            "bad signature",
        );
        assert!(matches!(
            SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
            Err(NeutralError::Context {
                problem: ContextProblem::Signature,
                ..
            })
        ));
    }
}
#[test]
fn admission_witness_rejects_mutated_decoded_info_and_replaced_raw_info() {
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx).unwrap().world.info.constant_b = Some(257);
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedInfo,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    let other = context(257, COMMON, MAP, "another");
    Arc::get_mut(&mut ctx).unwrap().world.info = other.world.info.clone();
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedInfo,
            ..
        })
    ));
}
#[test]
fn source_receipt_scripts_and_missing_witness_cannot_be_substituted() {
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx).unwrap().map_digest.label = "other".into();
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedAdmission,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    let other = context(257, COMMON, MAP, "other");
    Arc::get_mut(&mut ctx).unwrap().scripts = Arc::clone(&other.scripts);
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedAdmission,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx).unwrap().world_member_digests.clear();
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::MissingInfoReceipt,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx).unwrap().admission_witness = None;
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::MissingAdmission,
            ..
        })
    ));
}
#[test]
fn caller_labels_and_modified_pe_do_not_authenticate_unreviewed_input() {
    let mut i = input(257);
    i.profile[55] ^= 1;
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::UnapprovedProfile))
    ));
    let mut i = input(257);
    i.image[0x405] ^= 1;
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::ImageIdentity))
    ));
}
#[test]
fn operands_come_from_supplied_approved_pe_not_source_constants() {
    let a = input(257).import().unwrap();
    let b = input(1027).import().unwrap();
    assert_ne!(a.threshold, b.threshold);
    let ctx = context(300, COMMON, MAP, "selected");
    let mut first =
        SelectedMapNeutralHost::try_new(Arc::clone(&ctx), Arc::new(a), Delegate::default())
            .unwrap();
    let mut second =
        SelectedMapNeutralHost::try_new(ctx, Arc::new(b), Delegate::default()).unwrap();
    assert_eq!(first.native(NATIVE_NAME, &[]).unwrap(), Value::Int(61));
    assert_eq!(second.native(NATIVE_NAME, &[]).unwrap(), Value::Int(41));
}
#[test]
fn uninitialized_mapping_bad_binding_and_range_identity_are_refused() {
    let mut i = input(257);
    let profile_windows = i.profile.len() - 12 - 5 * 40;
    // An independently reapproved synthetic profile cannot authorize stale range bytes.
    i.image[0x400] ^= 1;
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::RangeIdentity))
    ));
    let mut i = input(257);
    let end = i.profile.len();
    i.profile[end - 12..end - 8].copy_from_slice(&2u32.to_le_bytes());
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::Binding))
    ));
    let mut i = input(257);
    i.image[0xf8 + 36..0xf8 + 40].copy_from_slice(&0x6000_0080u32.to_le_bytes());
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::Range))
    ));
    // Virtual tail has no raw bytes; a reviewed locator may not invent loader data.
    let mut i = input(257);
    i.image[0xf8 + 8..0xf8 + 12].copy_from_slice(&0x1800u32.to_le_bytes());
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    let name_window = profile_windows + 40;
    i.profile[name_window..name_window + 4].copy_from_slice(&0x0030_2100u32.to_le_bytes());
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::Range))
    ));
}
#[test]
fn approved_malformed_pe_and_input_budget_fail_closed() {
    let mut i = input(257);
    i.image[0] = 0;
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::PeFormat))
    ));
    let i = input(257);
    assert!(matches!(
        VerifiedNeutralRule::import(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len() - 1,
                profile_bytes: i.profile.len(),
                sections: 2
            }
        ),
        Err(NeutralError::Rule(RuleProblem::Budget))
    ));
}
#[test]
fn declared_metadata_mutation_and_signature_bytes_cannot_change_reviewed_binding() {
    let mut i = input(257);
    let window_start = i.profile.len() - 12 - 5 * 40;
    let raw_name = 0x300usize;
    i.image[raw_name] = b'X';
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    let n = NATIVE_NAME.len() + 1;
    i.profile[window_start + 40 + 8..window_start + 40 + 40]
        .copy_from_slice(&sha256(&i.image[raw_name..raw_name + n]));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::Binding))
    ));
    let mut i = input(257);
    let window_start = i.profile.len() - 12 - 5 * 40;
    i.image[0x342] = b'R';
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.profile[window_start + 2 * 40 + 8..window_start + 3 * 40]
        .copy_from_slice(&sha256(&i.image[0x340..0x344]));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::Binding))
    ));
}
#[test]
fn missing_native_fallback_is_never_delegated_and_other_context_provenance_is_rejected() {
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx).unwrap().sources.map.source = "unadmitted label".into();
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedAdmission,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    Arc::get_mut(&mut ctx)
        .unwrap()
        .world_member_digests
        .push(crate::scripts::SourceDigest {
            label: "selected:war3map.w3i".into(),
            hash: 0,
        });
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedInfo,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    assert!(
        Arc::get_mut(&mut Arc::get_mut(&mut ctx).unwrap().scripts).is_none(),
        "witness shares exact scripts Arc"
    );
}
#[test]
fn applicability_is_source_derived_and_relabelled_fnv_cannot_rebind_raw_info() {
    let mut i = input(257);
    i.image[0x500 + 14] = 30;
    let windows = i.profile.len() - 12 - 5 * 40;
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.profile[windows + 4 * 40 + 8..windows + 5 * 40]
        .copy_from_slice(&sha256(&i.image[0x500..0x500 + header().len()]));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    let r = Arc::new(i.import().unwrap());
    assert!(matches!(
        SelectedMapNeutralHost::try_new(
            context(256, COMMON, MAP, "selected"),
            r,
            Delegate::default()
        ),
        Err(NeutralError::Context {
            problem: ContextProblem::UnsupportedHeader,
            ..
        })
    ));
    let mut ctx = context(256, COMMON, MAP, "selected");
    let other = context(257, COMMON, MAP, "other");
    let edit = Arc::get_mut(&mut ctx).unwrap();
    edit.world.info = other.world.info.clone();
    let hash = pcx_script::fnv1a64(edit.world.info.raw());
    edit.world_member_digests
        .iter_mut()
        .find(|r| r.label == "selected:war3map.w3i")
        .unwrap()
        .hash = hash;
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, rules(), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::ChangedInfo,
            ..
        })
    ));
}
#[test]
fn adapter_satisfies_app_host_thread_bounds() {
    fn check<T: Send + Sync>() {}
    check::<SelectedMapNeutralHost<Delegate>>();
}

#[test]
fn approved_getter_mutation_and_ambiguous_header_are_not_generalized() {
    let mut i = input(257);
    let windows = i.profile.len() - 12 - 5 * 40;
    let getter_len = getter(257, -20, 61).len();
    i.image[0x400] = 0x90;
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    i.profile[windows + 3 * 40 + 8..windows + 4 * 40]
        .copy_from_slice(&sha256(&i.image[0x400..0x400 + getter_len]));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::GetterShape))
    ));

    let mut i = input(257);
    let h = header();
    i.image[0x500 + h.len()..0x500 + h.len() * 2].copy_from_slice(&h);
    i.profile[16..48].copy_from_slice(&sha256(&i.image));
    let header_window = i.profile.len() - 12 - 40;
    i.profile[header_window + 4..header_window + 8]
        .copy_from_slice(&(u32::try_from(h.len() * 2).unwrap()).to_le_bytes());
    i.profile[header_window + 8..header_window + 40]
        .copy_from_slice(&sha256(&i.image[0x500..0x500 + h.len() * 2]));
    i.approval = LocalProfileApproval::from_reviewed_fingerprint(sha256(&i.profile));
    assert!(matches!(
        i.import(),
        Err(NeutralError::Rule(RuleProblem::HeaderShape))
    ));
}

#[test]
fn linked_sha256_matches_independent_standard_vector() {
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];
    assert_eq!(sha256(b"abc"), expected);
}

fn aggressive_rules(threshold: u32) -> Arc<VerifiedNeutralRule> {
    let i = input_named(threshold, AGGRESSIVE_NATIVE_NAME);
    Arc::new(
        VerifiedNeutralRule::import_aggressive(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len(),
                profile_bytes: i.profile.len(),
                sections: 2,
            },
        )
        .unwrap(),
    )
}
#[test]
fn named_rule_imports_refuse_a_different_native_binding() {
    let names = [
        NATIVE_NAME,
        AGGRESSIVE_NATIVE_NAME,
        BJ_MAX_PLAYERS_NATIVE_NAME,
        BJ_NEUTRAL_VICTIM_NATIVE_NAME,
        BJ_NEUTRAL_EXTRA_NATIVE_NAME,
        BJ_MAX_PLAYER_SLOTS_NATIVE_NAME,
    ];
    let importers = [
        VerifiedNeutralRule::import,
        VerifiedNeutralRule::import_aggressive,
        VerifiedNeutralRule::import_bj_max_players_first_family,
        VerifiedNeutralRule::import_bj_neutral_victim_first_family,
        VerifiedNeutralRule::import_bj_neutral_extra_first_family,
        VerifiedNeutralRule::import_bj_max_player_slots_first_family,
    ];
    for (source, name) in names.iter().enumerate() {
        let i = input_named(257, name);
        let budget = RuleInputBudget {
            image_bytes: i.image.len(),
            profile_bytes: i.profile.len(),
            sections: 2,
        };
        for (target, importer) in importers.iter().enumerate() {
            let result = importer(&i.image, &i.profile, &i.approval, budget);
            if source == target {
                assert_eq!(result.unwrap().native_name, *name);
            } else {
                assert!(matches!(
                    result,
                    Err(NeutralError::Rule(RuleProblem::Binding))
                ));
            }
        }
    }
}
#[test]
fn composed_named_hosts_use_separate_input_rules_before_global_initialization() {
    let common = format!(
        "{COMMON}native {AGGRESSIVE_NATIVE_NAME} takes nothing returns integer\nnative {BJ_MAX_PLAYERS_NATIVE_NAME} takes nothing returns integer\nnative {BJ_NEUTRAL_VICTIM_NATIVE_NAME} takes nothing returns integer\nnative {BJ_NEUTRAL_EXTRA_NATIVE_NAME} takes nothing returns integer\nnative {BJ_MAX_PLAYER_SLOTS_NATIVE_NAME} takes nothing returns integer\n"
    );
    let map = format!(
        "globals\ninteger passive = {NATIVE_NAME}()\ninteger aggressive = {AGGRESSIVE_NATIVE_NAME}()\ninteger limit = {BJ_MAX_PLAYERS_NATIVE_NAME}()\ninteger victim = {BJ_NEUTRAL_VICTIM_NATIVE_NAME}()\ninteger extra = {BJ_NEUTRAL_EXTRA_NATIVE_NAME}()\ninteger slots = {BJ_MAX_PLAYER_SLOTS_NATIVE_NAME}()\nendglobals\n"
    );
    let ctx = context(300, &common, &map, "selected-both");
    let slots = SelectedMapNeutralHost::try_new(
        Arc::clone(&ctx),
        slots_rule(299, -31, 19),
        Delegate::default(),
    )
    .unwrap();
    let extra =
        SelectedMapNeutralHost::try_new(Arc::clone(&ctx), extra_rule(301, 11, -47), slots).unwrap();
    let victim =
        SelectedMapNeutralHost::try_new(Arc::clone(&ctx), victim_rule(65539, -53, 83), extra)
            .unwrap();
    let aggressive =
        SelectedMapNeutralHost::try_new(Arc::clone(&ctx), aggressive_rules(1027), victim).unwrap();
    let limit = SelectedMapNeutralHost::try_new(
        Arc::clone(&ctx),
        bj_max_players_rule(503, 29, -67),
        aggressive,
    )
    .unwrap();
    let host = SelectedMapNeutralHost::try_new(Arc::clone(&ctx), rules(), limit).unwrap();
    assert!(Arc::ptr_eq(host.context(), host.inner().context()));
    assert!(Arc::ptr_eq(host.context(), host.inner().inner().context()));
    assert!(Arc::ptr_eq(
        &ctx.scripts.shared_program(),
        &host.inner().context().scripts.shared_program()
    ));
    assert_eq!(host.map_sha256(), host.inner().map_sha256());
    assert_eq!(host.info_sha256(), host.inner().info_sha256());
    assert_eq!(host.map_sha256(), host.inner().inner().map_sha256());
    assert_eq!(host.info_sha256(), host.inner().inner().info_sha256());
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    assert!(Arc::ptr_eq(vm.host().context(), &ctx.scripts));
    vm.init_globals().unwrap();
    assert_eq!(vm.global("passive"), Value::Int(61));
    assert_eq!(vm.global("aggressive"), Value::Int(41));
    assert_eq!(vm.global("limit"), Value::Int(-38));
    assert_eq!(vm.global("victim"), Value::Int(30));
    assert_eq!(vm.global("extra"), Value::Int(-36));
    assert_eq!(vm.global("slots"), Value::Int(19));
}
#[test]
fn aggressive_host_preserves_unsigned_boundaries_signature_args_and_delegation() {
    let common = format!(
        "native {AGGRESSIVE_NATIVE_NAME} takes nothing returns integer\nnative Other takes nothing returns integer\n"
    );
    for (word, expected) in [(256, 41), (257, 61), (u32::MAX, 61)] {
        let ctx = context(word, &common, "// map", "selected-aggressive");
        let mut host =
            SelectedMapNeutralHost::try_new(ctx, aggressive_rules(257), Delegate::default())
                .unwrap();
        assert_eq!(
            host.native(AGGRESSIVE_NATIVE_NAME, &[]).unwrap(),
            Value::Int(expected)
        );
        assert!(
            host.native(AGGRESSIVE_NATIVE_NAME, &[Value::Int(1)])
                .unwrap_err()
                .contains("Arguments")
        );
        assert_eq!(host.native("Other", &[]).unwrap(), Value::Int(313));
        assert!(
            host.native(NATIVE_NAME, &[])
                .unwrap_err()
                .contains("delegate refusal")
        );
    }
    let ctx = context(256, COMMON, MAP, "missing-aggressive-declaration");
    assert!(matches!(
        SelectedMapNeutralHost::try_new(ctx, aggressive_rules(257), Delegate::default()),
        Err(NeutralError::Context {
            problem: ContextProblem::Signature,
            ..
        })
    ));
}

fn bj_max_players_rule(threshold: u32, mask: i8, add: i8) -> Arc<VerifiedNeutralRule> {
    let i = input_named_operands(threshold, BJ_MAX_PLAYERS_NATIVE_NAME, mask, add);
    Arc::new(
        VerifiedNeutralRule::import_bj_max_players_first_family(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len(),
                profile_bytes: i.profile.len(),
                sections: 2,
            },
        )
        .unwrap(),
    )
}

#[test]
fn bj_max_players_first_family_derives_its_own_unsigned_boundary_and_signed_results() {
    for (word, expected) in [(502, -38), (503, -67), (504, -67), (u32::MAX, -67)] {
        let common = format!(
            "native {BJ_MAX_PLAYERS_NATIVE_NAME} takes nothing returns integer\nnative Other takes nothing returns integer\n"
        );
        let ctx = context(word, &common, "// synthetic map", "selected-player-limit");
        let mut host = SelectedMapNeutralHost::try_new(
            Arc::clone(&ctx),
            bj_max_players_rule(503, 29, -67),
            Delegate::default(),
        )
        .unwrap();
        assert!(Arc::ptr_eq(host.context(), &ctx));
        assert_eq!(
            host.native(BJ_MAX_PLAYERS_NATIVE_NAME, &[]).unwrap(),
            Value::Int(expected)
        );
        let error = host
            .native(BJ_MAX_PLAYERS_NATIVE_NAME, &[Value::Int(1)])
            .unwrap_err();
        assert!(error.contains("Arguments") && error.contains("selected-player-limit"));
        assert_eq!(host.native("Other", &[]).unwrap(), Value::Int(313));
        assert!(
            host.native(NATIVE_NAME, &[])
                .unwrap_err()
                .contains("delegate refusal")
        );
        assert!(
            host.native(AGGRESSIVE_NATIVE_NAME, &[])
                .unwrap_err()
                .contains("delegate refusal")
        );
        assert_eq!(host.inner().calls, 3);
        assert_eq!(
            host.resolve_table_string(9),
            Err("table read failure".into())
        );
        assert_eq!(host.table_string(2), Some("table 2".into()));
        assert_eq!(host.code_tag("x"), Some("tag x".into()));
    }
}

#[test]
fn bj_max_players_first_family_requires_approval_of_that_exact_profile_and_image() {
    let selected = input_named_operands(503, BJ_MAX_PLAYERS_NATIVE_NAME, 29, -67);
    let mut other = input_named_operands(997, BJ_MAX_PLAYERS_NATIVE_NAME, -37, 57);
    let budget = RuleInputBudget {
        image_bytes: other.image.len(),
        profile_bytes: other.profile.len(),
        sections: 2,
    };
    assert!(matches!(
        VerifiedNeutralRule::import_bj_max_players_first_family(
            &other.image,
            &other.profile,
            &selected.approval,
            budget
        ),
        Err(NeutralError::Rule(RuleProblem::UnapprovedProfile))
    ));
    other.image[0x405] ^= 1;
    assert!(matches!(
        VerifiedNeutralRule::import_bj_max_players_first_family(
            &other.image,
            &other.profile,
            &other.approval,
            budget
        ),
        Err(NeutralError::Rule(RuleProblem::ImageIdentity))
    ));
}

#[test]
fn bj_max_players_first_family_requires_its_same_context_program_signature() {
    for common in [
        "// no declaration\n".into(),
        format!("native {BJ_MAX_PLAYERS_NATIVE_NAME} takes nothing returns real\n"),
        format!("native {BJ_MAX_PLAYERS_NATIVE_NAME} takes integer x returns integer\n"),
    ] {
        let ctx = context(
            503,
            &common,
            "// synthetic map",
            "bad-player-limit-signature",
        );
        assert!(matches!(
            SelectedMapNeutralHost::try_new(ctx, bj_max_players_rule(503, 29, -67), Delegate::default()),
            Err(NeutralError::Context { map, problem: ContextProblem::Signature }) if map == "bad-player-limit-signature"
        ));
    }
}

#[test]
fn bj_max_players_initializer_keeps_next_native_diagnostic_and_sticky_failure() {
    let common = format!(
        "native {BJ_MAX_PLAYERS_NATIVE_NAME} takes nothing returns integer\nnative Missing takes nothing returns integer\n"
    );
    let map = format!(
        "globals\ninteger limit = {BJ_MAX_PLAYERS_NATIVE_NAME}()\ninteger next = Missing()\nendglobals\n"
    );
    let ctx = context(502, &common, &map, "selected-player-limit-frontier");
    let host = SelectedMapNeutralHost::try_new(
        Arc::clone(&ctx),
        bj_max_players_rule(503, 29, -67),
        Delegate::default(),
    )
    .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    let first = vm.init_globals().unwrap_err();
    let source = vm.error_source().unwrap().to_owned();
    let diagnostic = vm.host().diagnostic(Some(&source), &first);
    let initializer_source = &ctx
        .scripts
        .program()
        .globals()
        .iter()
        .find(|global| global.name == "next")
        .unwrap()
        .source;
    assert_eq!(&source, initializer_source);
    assert!(
        diagnostic.contains("Missing") && diagnostic.contains("selected-player-limit-frontier")
    );
    assert_eq!(vm.global("limit"), Value::Int(-38));
    assert_eq!(vm.host().inner.inner().calls, 1);
    assert_eq!(first, vm.init_globals().unwrap_err());
    assert_eq!(vm.error_source(), Some(source.as_str()));
    assert_eq!(vm.host().diagnostic(Some(&source), &first), diagnostic);
    assert_eq!(vm.host().inner.inner().calls, 1);
}

fn victim_rule(threshold: u32, mask: i8, add: i8) -> Arc<VerifiedNeutralRule> {
    let i = input_named_operands(threshold, BJ_NEUTRAL_VICTIM_NATIVE_NAME, mask, add);
    Arc::new(
        VerifiedNeutralRule::import_bj_neutral_victim_first_family(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len(),
                profile_bytes: i.profile.len(),
                sections: 2,
            },
        )
        .unwrap(),
    )
}

#[test]
fn victim_getter_derives_its_own_signed_results_and_unsigned_boundary() {
    let common = format!(
        "native {BJ_NEUTRAL_VICTIM_NATIVE_NAME} takes nothing returns integer\nnative Other takes nothing returns integer\n"
    );
    for (word, expected) in [(0, 30), (65538, 30), (65539, 83), (u32::MAX, 83)] {
        let ctx = context(word, &common, "// map", "selected-victim");
        let mut host = SelectedMapNeutralHost::try_new(
            Arc::clone(&ctx),
            victim_rule(65539, -53, 83),
            Delegate::default(),
        )
        .unwrap();
        assert!(Arc::ptr_eq(host.context(), &ctx));
        assert_eq!(
            host.native(BJ_NEUTRAL_VICTIM_NATIVE_NAME, &[]).unwrap(),
            Value::Int(expected)
        );
        assert!(
            host.native(BJ_NEUTRAL_VICTIM_NATIVE_NAME, &[Value::Int(9)])
                .unwrap_err()
                .contains("Arguments")
        );
        assert_eq!(host.native("Other", &[]).unwrap(), Value::Int(313));
        assert_eq!(host.inner().calls, 1);
    }
}

#[test]
fn victim_getter_requires_the_same_context_zero_argument_integer_declaration() {
    for common in [
        "// absent declaration\n".into(),
        format!("native {BJ_NEUTRAL_VICTIM_NATIVE_NAME} takes nothing returns real\n"),
        format!("native {BJ_NEUTRAL_VICTIM_NATIVE_NAME} takes integer x returns integer\n"),
    ] {
        let ctx = context(65538, &common, "// map", "bad-victim-signature");
        assert!(matches!(
            SelectedMapNeutralHost::try_new(ctx, victim_rule(65539, -53, 83), Delegate::default()),
            Err(NeutralError::Context { map, problem: ContextProblem::Signature }) if map == "bad-victim-signature"
        ));
    }
}

#[test]
fn victim_initializer_preserves_localization_owner_and_sticky_next_native_refusal() {
    let common = format!(
        "native {BJ_NEUTRAL_VICTIM_NATIVE_NAME} takes nothing returns integer\nnative Missing takes nothing returns integer\n"
    );
    let map = format!(
        "globals\ninteger victim = {BJ_NEUTRAL_VICTIM_NATIVE_NAME}()\ninteger next = Missing()\nendglobals\n"
    );
    let ctx = context(65538, &common, &map, "victim-initializer");
    let host = SelectedMapNeutralHost::try_new(
        Arc::clone(&ctx),
        victim_rule(65539, -53, 83),
        Delegate::default(),
    )
    .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    assert!(Arc::ptr_eq(vm.host().context(), &ctx.scripts));
    let first = vm.init_globals().unwrap_err();
    let source = vm.error_source().unwrap().to_owned();
    let diagnostic = vm.host().diagnostic(Some(&source), &first);
    assert!(diagnostic.contains("Missing") && diagnostic.contains("victim-initializer"));
    assert_eq!(vm.global("victim"), Value::Int(30));
    assert_eq!(vm.host().inner.inner().calls, 1);
    assert_eq!(first, vm.init_globals().unwrap_err());
    assert_eq!(vm.host().diagnostic(Some(&source), &first), diagnostic);
    assert_eq!(vm.host().inner.inner().calls, 1);
}

fn extra_rule(threshold: u32, mask: i8, add: i8) -> Arc<VerifiedNeutralRule> {
    let i = input_named_operands(threshold, BJ_NEUTRAL_EXTRA_NATIVE_NAME, mask, add);
    Arc::new(
        VerifiedNeutralRule::import_bj_neutral_extra_first_family(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len(),
                profile_bytes: i.profile.len(),
                sections: 2,
            },
        )
        .unwrap(),
    )
}

#[test]
fn extra_getter_uses_independent_operands_and_preserves_next_failure() {
    let common = format!(
        "native {BJ_NEUTRAL_EXTRA_NATIVE_NAME} takes nothing returns integer\nnative Missing takes nothing returns integer\n"
    );
    let map = format!(
        "globals\ninteger extra = {BJ_NEUTRAL_EXTRA_NATIVE_NAME}()\ninteger next = Missing()\nendglobals\n"
    );
    for (word, expected) in [(0, -36), (300, -36), (301, -47), (u32::MAX, -47)] {
        let ctx = context(word, &common, &map, "extra-initializer");
        let host = SelectedMapNeutralHost::try_new(
            Arc::clone(&ctx),
            extra_rule(301, 11, -47),
            Delegate::default(),
        )
        .unwrap();
        let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
        assert!(Arc::ptr_eq(vm.host().context(), &ctx.scripts));
        let first = vm.init_globals().unwrap_err();
        let source = vm.error_source().unwrap().to_owned();
        let diagnostic = vm.host().diagnostic(Some(&source), &first);
        assert!(diagnostic.contains("Missing") && diagnostic.contains("extra-initializer"));
        assert_eq!(vm.global("extra"), Value::Int(expected));
        assert_eq!(vm.host().inner.inner().calls, 1);
        assert_eq!(first, vm.init_globals().unwrap_err());
        assert_eq!(vm.host().diagnostic(Some(&source), &first), diagnostic);
        assert_eq!(vm.host().inner.inner().calls, 1);
    }
}

fn slots_rule(threshold: u32, mask: i8, add: i8) -> Arc<VerifiedNeutralRule> {
    let i = input_named_operands(threshold, BJ_MAX_PLAYER_SLOTS_NATIVE_NAME, mask, add);
    Arc::new(
        VerifiedNeutralRule::import_bj_max_player_slots_first_family(
            &i.image,
            &i.profile,
            &i.approval,
            RuleInputBudget {
                image_bytes: i.image.len(),
                profile_bytes: i.profile.len(),
                sections: 2,
            },
        )
        .unwrap(),
    )
}

#[test]
fn slot_getter_checks_independent_boundary_arguments_and_same_context_signature() {
    let common =
        format!("native {BJ_MAX_PLAYER_SLOTS_NATIVE_NAME} takes nothing returns integer\n");
    for (word, expected) in [(0, -12), (298, -12), (299, 19), (u32::MAX, 19)] {
        let ctx = context(word, &common, "// map", "slot-getter");
        let mut host =
            SelectedMapNeutralHost::try_new(ctx, slots_rule(299, -31, 19), Delegate::default())
                .unwrap();
        assert_eq!(
            host.native(BJ_MAX_PLAYER_SLOTS_NATIVE_NAME, &[]).unwrap(),
            Value::Int(expected)
        );
        assert!(
            host.native(BJ_MAX_PLAYER_SLOTS_NATIVE_NAME, &[Value::Int(1)])
                .unwrap_err()
                .contains("Arguments")
        );
    }
    for common in [
        "// absent".to_owned(),
        format!("native {BJ_MAX_PLAYER_SLOTS_NATIVE_NAME} takes nothing returns real\n"),
        format!("native {BJ_MAX_PLAYER_SLOTS_NATIVE_NAME} takes integer x returns integer\n"),
    ] {
        let ctx = context(299, &common, "// map", "bad-slot-signature");
        assert!(matches!(
            SelectedMapNeutralHost::try_new(ctx, slots_rule(299, -31, 19), Delegate::default()),
            Err(NeutralError::Context {
                problem: ContextProblem::Signature,
                ..
            })
        ));
    }
}

#[path = "player_host_tests.rs"]
mod player_objects;

#[test]
fn cooperative_delegate_cannot_park_the_owned_neutral_getter() {
    struct Park {
        names: Vec<String>,
    }
    impl Host for Park {
        fn suspend(&mut self, name: &str, _: &[Value]) -> bool {
            self.names.push(name.into());
            true
        }
        fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("native must remain parked".into())
        }
    }
    let ctx = context(256, COMMON, MAP, "cooperative delegate");
    let mut host =
        SelectedMapNeutralHost::try_new(ctx, rules(), Park { names: Vec::new() }).unwrap();
    assert!(!host.suspend(NATIVE_NAME, &[]));
    assert!(host.inner().names.is_empty());
    assert!(matches!(
        host.native(NATIVE_NAME, &[]).unwrap(),
        Value::Int(_)
    ));
    assert!(host.suspend("SyntheticPark", &[Value::Int(4)]));
    assert_eq!(host.inner().names, ["SyntheticPark"]);
}
