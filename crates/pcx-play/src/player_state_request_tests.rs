use super::{COMMON, budget, context, id, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost, PlayerStateRequestBudget};
use pcx_script::{Host, Value};
use std::sync::Arc;

const SETTER: &str =
    "native SetPlayerState takes player p, playerstate setting, integer value returns nothing";
const GETTER: &str =
    "constant native GetPlayerState takes player p, playerstate setting returns integer\n";
fn common() -> String {
    COMMON.replacen("globals\n", &format!("type playerstate extends handle\nconstant native ConvertPlayerState takes integer i returns playerstate\nconstant native ConvertOtherState takes integer i returns playerstate\n{SETTER}\n{GETTER}globals\nconstant playerstate SX = ConvertPlayerState(-11)\nconstant playerstate SY = ConvertPlayerState(37)\nconstant playerstate SZ = ConvertOtherState(999)\n"), 1)
}
fn fixture(common: &str, map: &str, selected: bool) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = context(301, common, map, "synthetic-player-state-requests");
    let mut names = vec![
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
    ];
    if selected {
        names.extend(["ConvertPlayerState", "ConvertOtherState"]);
        if ctx
            .scripts
            .program()
            .natives()
            .get("ConvertVersion")
            .is_some()
        {
            names.push("ConvertVersion");
        }
    }
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &names,
        Limits {
            max_handles: 32,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(enums)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    (ctx, host)
}
#[test]
fn owned_vm_retains_signed_requests_without_effective_defaults_or_getters() {
    let c = common();
    let map = "globals\nplayer A = Player(0)\nplayer Alias = Player(0)\nplayer B = Player(1)\nendglobals\nfunction request takes player p, playerstate setting, integer value returns nothing\ncall SetPlayerState(p,setting,value)\nendfunction\n";
    let (ctx, host) = fixture(&c, map, true);
    let host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 3 })
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    drop(ctx);
    drop(c);
    vm.init_globals().unwrap();
    let a = vm.global("A");
    let alias = vm.global("Alias");
    let b = vm.global("B");
    let x = vm.global("SX");
    let y = vm.global("SY");
    assert_eq!(a, alias);
    assert!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    let handles = vm.host().inner.inner().handles();
    for args in [
        vec![a.clone(), x.clone(), Value::Int(0)],
        vec![a.clone(), y.clone(), Value::Int(-23)],
        vec![b.clone(), x.clone(), Value::Int(i32::MAX)],
    ] {
        vm.call("request", &args).unwrap();
    }
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests,
        vec![(id(&x), 0), (id(&y), -23)]
    );
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&b))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MAX)]
    );
    vm.call("request", &[alias, x.clone(), Value::Int(i32::MIN)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&a))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MIN), (id(&y), -23)]
    );
    let error = vm
        .call("request", &[b.clone(), y, Value::Int(0)])
        .unwrap_err();
    assert!(
        error.to_string().contains("player-state requests"),
        "{error}"
    );
    assert_eq!(
        vm.host()
            .inner
            .player_settings(id(&b))
            .unwrap()
            .state_requests,
        vec![(id(&x), i32::MAX)]
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert!(
        vm.host()
            .inner
            .native("GetPlayerState", &[a, x])
            .unwrap_err()
            .contains("unsupported")
    );
}
#[test]
fn refused_requests_preserve_prior_rows_and_zero_budget_allocates_nothing() {
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(37)])
        .unwrap();
    let color = host
        .native("ConvertPlayerColor", &[Value::Int(37)])
        .unwrap();
    let outside = host
        .native("ConvertOtherState", &[Value::Int(999)])
        .unwrap();
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(u64::MAX).unwrap(),
    ));
    host.native("SetPlayerState", &[a.clone(), x.clone(), Value::Int(0)])
        .unwrap();
    let handles = host.inner().handles();
    for args in [
        vec![],
        vec![a.clone(), x.clone(), Value::Real(0.)],
        vec![color.clone(), x.clone(), Value::Int(1)],
        vec![a.clone(), color, Value::Int(1)],
        vec![a.clone(), outside, Value::Int(1)],
        vec![unknown.clone(), x.clone(), Value::Int(1)],
        vec![a.clone(), unknown, Value::Int(1)],
    ] {
        assert!(host.native("SetPlayerState", &args).is_err());
        assert_eq!(
            host.player_settings(id(&a)).unwrap().state_requests,
            vec![(id(&x), 0)]
        );
        assert_eq!(host.inner().handles(), handles);
    }
    let (_, host) = fixture(&common(), "// map", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 0 })
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(-11)])
        .unwrap();
    let handles = host.inner().handles();
    assert!(
        host.native("SetPlayerState", &[a.clone(), x, Value::Int(0)])
            .is_err()
    );
    assert!(
        host.player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    assert_eq!(host.inner().handles(), handles);
}
#[test]
fn request_admission_is_explicit_source_bound_and_independent_of_the_getter() {
    let c = common();
    let (_, mut host) = fixture(&c, "// map", true);
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let x = host
        .native("ConvertPlayerState", &[Value::Int(-11)])
        .unwrap();
    assert!(
        host.native("SetPlayerState", &[a.clone(), x, Value::Int(0)])
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(
        host.player_settings(id(&a))
            .unwrap()
            .state_requests
            .is_empty()
    );
    let (_, host) = fixture(&c.replace(GETTER, ""), "// map", true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_ok()
    );
    for c in [
        c.replace(SETTER, ""),
        c.replace(SETTER, &format!("constant {SETTER}")),
        c.replace(
            "playerstate setting, integer value returns nothing",
            "playerstate setting, real value returns nothing",
        ),
    ] {
        let (_, host) = fixture(&c, "// map", true);
        assert!(
            host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
                .is_err()
        );
    }
    for (c, map, selected) in [
        (&c, "// map".to_string(), false),
        (&c, format!("{SETTER}\n"), true),
    ] {
        let (_, host) = fixture(c, &map, selected);
        assert!(
            host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
                .is_err()
        );
    }
    let (_, host) = fixture(&c.replace(SETTER, ""), &format!("{SETTER}\n"), true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    let host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
        .unwrap();
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
    let (_, host) = fixture(&c, "// map", true);
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget {
            max_records: usize::MAX
        })
        .is_err()
    );
    let ctx = context(301, &c, "// map", "no-players");
    let host = ObjectHost::checked(super::enums(&ctx, 32)).unwrap();
    assert!(
        host.with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
            .is_err()
    );
}

fn resource_common() -> String {
    common().replacen("globals\n", "type version extends handle\nconstant native ConvertVersion takes integer i returns version\nnative VersionGet takes nothing returns version\ntype widget extends agent\ntype unit extends widget\nnative CreateUnit takes player p, integer id, real x, real y, real face returns unit\nnative RemoveUnit takes unit u returns nothing\nnative SetUnitOwner takes unit u, player p, boolean color returns nothing\nglobals\nconstant version RESOURCE_VERSION_LOW=ConvertVersion(0)\nconstant version RESOURCE_VERSION_HIGH=ConvertVersion(1)\nconstant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)\nconstant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)\nconstant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)\nconstant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)\n",1)
}
#[test]
fn effective_money_uses_selected_initial_scale_and_clamp_while_requests_stay_exact() {
    let c = resource_common();
    let (_, host) = fixture(&c, "// resources", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 1 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let b = host.native("Player", &[Value::Int(1)]).unwrap();
    let gold = host.native("ConvertPlayerState", &[Value::Int(1)]).unwrap();
    let lumber = host.native("ConvertPlayerState", &[Value::Int(2)]).unwrap();
    assert_eq!(
        host.player_resource_value(id(&a), "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(3)
    );
    assert_eq!(
        host.native("GetPlayerState", &[a.clone(), gold.clone()])
            .unwrap(),
        Value::Int(3)
    );
    host.native(
        "SetPlayerState",
        &[a.clone(), gold.clone(), Value::Int(117)],
    )
    .unwrap();
    assert_eq!(
        host.native("GetPlayerState", &[a.clone(), gold.clone()])
            .unwrap(),
        Value::Int(100)
    );
    assert_eq!(
        host.player_settings(id(&a)).unwrap().state_requests,
        [(id(&gold), 117)]
    );
    assert!(
        host.native(
            "SetPlayerState",
            &[a.clone(), lumber.clone(), Value::Int(37)]
        )
        .is_err()
    );
    assert_eq!(
        host.native("GetPlayerState", &[a.clone(), lumber]).unwrap(),
        Value::Int(3)
    );
    assert_eq!(
        host.native("GetPlayerState", &[b, gold.clone()]).unwrap(),
        Value::Int(3)
    );
    host.native("SetPlayerState", &[a.clone(), gold.clone(), Value::Int(-3)])
        .unwrap();
    assert_eq!(
        host.native("GetPlayerState", &[a, gold]).unwrap(),
        Value::Int(0)
    );
}

struct FoodCatalog;
impl pcx_sim::world::ObjectCatalog for FoodCatalog {
    fn resolve(
        &self,
        rawcode: [u8; 4],
    ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
        use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
        use pcx_sim::world::{FieldValue, ObjectField};
        let fields = [(*b"f001", "fused", 2), (*b"f002", "fmade", 5)].map(|(id, column, value)| {
            (
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: column.into(),
                        slk: "UnitBalance".into(),
                        index: -1,
                        declared: "int".into(),
                        kind: FieldKind::Int,
                        category: "synthetic".into(),
                        source: FieldSource::Column {
                            slk: SlkId::UnitBalance,
                            column: column.into(),
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(value.to_string())),
                },
            )
        });
        Ok(pcx_sim::world::Definition {
            rawcode,
            base_rawcode: pcx_objects::fourcc_text(rawcode),
            kind: ObjectKind::Unit,
            fields: fields.into(),
            modifications: Vec::new(),
        })
    }
}
#[test]
fn effective_food_follows_owned_unit_lifetimes_and_explicit_counter_adjustments() {
    use crate::playercolor_host::UnitBudget;
    let (_, host) = fixture(&resource_common(), "// food", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 4 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules())
        .unwrap()
        .with_unit_catalog(
            Arc::new(FoodCatalog),
            pcx_sim::world::Source {
                install: "synthetic catalog".into(),
                edition: "synthetic".into(),
                map: "synthetic food".into(),
            },
            UnitBudget {
                max_records: 4,
                max_retained_fields: 2,
                max_modifications: 0,
            },
        )
        .unwrap();
    let a = host.native("Player", &[Value::Int(0)]).unwrap();
    let b = host.native("Player", &[Value::Int(1)]).unwrap();
    let mut create = |owner| {
        host.native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"hfoo")),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    };
    let first = create(a.clone());
    let second = create(a.clone());
    let _third = create(b.clone());
    let used = "PLAYER_STATE_RESOURCE_FOOD_USED";
    let cap = "PLAYER_STATE_RESOURCE_FOOD_CAP";
    assert_eq!(host.player_resource_value(id(&a), used).unwrap(), Some(22));
    assert_eq!(host.player_resource_value(id(&a), cap).unwrap(), Some(28));
    let state = host.native("ConvertPlayerState", &[Value::Int(5)]).unwrap();
    host.native("SetPlayerState", &[a.clone(), state, Value::Int(30)])
        .unwrap();
    assert_eq!(host.player_resource_value(id(&a), used).unwrap(), Some(30));
    host.native("SetUnitOwner", &[first, b.clone(), Value::Bool(false)])
        .unwrap();
    assert_eq!(host.player_resource_value(id(&a), used).unwrap(), Some(28));
    assert_eq!(host.player_resource_value(id(&b), used).unwrap(), Some(22));
    host.native("RemoveUnit", &[second]).unwrap();
    assert_eq!(host.player_resource_value(id(&a), used).unwrap(), Some(26));
    assert_eq!(host.player_resource_value(id(&a), cap).unwrap(), Some(18));
    let ceiling = host.native("ConvertPlayerState", &[Value::Int(6)]).unwrap();
    host.native("SetPlayerState", &[b.clone(), ceiling, Value::Int(21)])
        .unwrap();
    assert_eq!(host.player_resource_value(id(&b), cap).unwrap(), Some(21));
}

#[test]
fn food_ceiling_waits_for_canonical_version_unless_authored_or_explicitly_assigned() {
    let (ctx, host) = fixture(&resource_common(), "// canonical resource version", true);
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 4 })
        .unwrap()
        .with_player_resources(crate::player_resources::fixture::rules_without_food_override())
        .unwrap();
    let player = host.native("Player", &[Value::Int(0)]).unwrap();
    let ceiling = "PLAYER_STATE_FOOD_CAP_CEILING";
    assert!(
        host.player_resource_value(id(&player), ceiling)
            .unwrap_err()
            .to_string()
            .contains("startup version")
    );
    let low = host.native("ConvertVersion", &[Value::Int(0)]).unwrap();
    host.admit_startup_version(&ctx, id(&low)).unwrap();
    assert_eq!(
        host.player_resource_value(id(&player), ceiling).unwrap(),
        Some(61)
    );
    let high = host.native("ConvertVersion", &[Value::Int(1)]).unwrap();
    host.admit_startup_version(&ctx, id(&high)).unwrap();
    assert_eq!(
        host.player_resource_value(id(&player), ceiling).unwrap(),
        Some(87)
    );
    host.invalidate_startup_version();
    let state = host.native("ConvertPlayerState", &[Value::Int(6)]).unwrap();
    host.native("SetPlayerState", &[player.clone(), state, Value::Int(37)])
        .unwrap();
    assert_eq!(
        host.player_resource_value(id(&player), ceiling).unwrap(),
        Some(37)
    );
}

struct ResolvedFoodCatalog(std::collections::BTreeMap<[u8; 4], pcx_sim::world::Definition>);
impl pcx_sim::world::ObjectCatalog for ResolvedFoodCatalog {
    fn resolve(
        &self,
        rawcode: [u8; 4],
    ) -> Result<pcx_sim::world::Definition, pcx_sim::world::ResolutionProblem> {
        self.0.get(&rawcode).cloned().ok_or_else(|| {
            pcx_sim::world::ResolutionProblem::MissingRawcode(pcx_objects::fourcc_text(rawcode))
        })
    }
}
fn marker_food_queries(
    catalog: ResolvedFoodCatalog,
    rules: crate::player_resources::Rules,
    invalid: bool,
) {
    use crate::playercolor_host::UnitBudget;
    let (ctx, host) = fixture(
        &resource_common(),
        "// resolved authored food markers",
        true,
    );
    let initial = rules.initial;
    let mut host = host
        .with_player_state_requests(PlayerStateRequestBudget { max_records: 4 })
        .unwrap()
        .with_player_resources(rules)
        .unwrap()
        .with_unit_catalog(
            Arc::new(catalog),
            pcx_sim::world::Source {
                install: "explicit supplied food catalog".into(),
                edition: "caller selected".into(),
                map: "resolved food regression".into(),
            },
            UnitBudget {
                max_records: 2,
                max_retained_fields: 20000,
                max_modifications: 0,
            },
        )
        .unwrap();
    let version = host.native("ConvertVersion", &[Value::Int(1)]).unwrap();
    host.admit_startup_version(&ctx, id(&version)).unwrap();
    let owner = host.native("Player", &[Value::Int(0)]).unwrap();
    for rawcode in [*b"Hart", *b"hfoo"] {
        host.native(
            "CreateUnit",
            &[
                owner.clone(),
                Value::Int(i32::from_be_bytes(rawcode)),
                Value::Real(0.),
                Value::Real(0.),
                Value::Real(0.),
            ],
        )
        .unwrap();
    }
    if invalid {
        assert!(
            host.player_resource_value(id(&owner), "PLAYER_STATE_RESOURCE_FOOD_USED")
                .is_err()
        );
        return;
    }
    assert_eq!(
        host.player_resource_value(id(&owner), "PLAYER_STATE_RESOURCE_FOOD_USED")
            .unwrap(),
        Some(initial + 2)
    );
    assert_eq!(
        host.player_resource_value(id(&owner), "PLAYER_STATE_RESOURCE_FOOD_CAP")
            .unwrap(),
        Some(initial)
    );
}
#[test]
fn authored_food_markers_use_admitted_conversion_and_malformed_values_refuse() {
    use pcx_sim::world::{FieldValue, ObjectCatalog};
    let mut definitions = std::collections::BTreeMap::new();
    for rawcode in [*b"Hart", *b"hfoo"] {
        let mut definition = FoodCatalog.resolve(rawcode).unwrap();
        for field in definition.fields.values_mut() {
            let column = field.metadata.as_ref().unwrap().field.as_str();
            field.value = Ok(FieldValue::Cell(
                if rawcode == *b"Hart" || column == "fmade" {
                    " - "
                } else {
                    " 2 "
                }
                .into(),
            ));
        }
        definitions.insert(rawcode, definition);
    }
    let mut malformed = definitions.clone();
    malformed
        .get_mut(b"Hart")
        .unwrap()
        .fields
        .get_mut(b"f001")
        .unwrap()
        .value = Ok(FieldValue::Cell("mistyped".into()));
    marker_food_queries(
        ResolvedFoodCatalog(definitions),
        crate::player_resources::fixture::rules(),
        false,
    );
    marker_food_queries(
        ResolvedFoodCatalog(malformed),
        crate::player_resources::fixture::rules(),
        true,
    );
}

#[test]
#[ignore = "requires PCX_PLAYER_RESOURCE_CATALOG selected source copy, PCX_PLAYER_RESOURCE_IMAGE and PCX_PLAYER_RESOURCE_PROFILE"]
fn supplied_resolved_human_units_support_normal_food_queries() {
    use pcx_sim::world::ObjectCatalog;
    let root = std::path::PathBuf::from(
        std::env::var("PCX_PLAYER_RESOURCE_CATALOG").expect("PCX_PLAYER_RESOURCE_CATALOG"),
    );
    let editor = pcx_objects::editor::Editor::from_reader(
        &root,
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 0,
            max_asset_bytes: 8 << 20,
            max_aggregate_input_bytes: 64 << 20,
        },
        |member| match std::fs::read(root.join(member.replace('\\', "/").to_ascii_lowercase())) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        },
    )
    .unwrap();
    let units = pcx_objects::editor::Overlay::default();
    let items = pcx_objects::editor::Overlay::default();
    let catalog = pcx_sim::world::MetadataCatalog {
        editor: &editor,
        units: &units,
        items: &items,
    };
    let definitions = [*b"Hart", *b"hfoo"]
        .into_iter()
        .map(|id| (id, catalog.resolve(id).unwrap()))
        .collect();
    let image_path = std::env::var("PCX_PLAYER_RESOURCE_IMAGE").expect("PCX_PLAYER_RESOURCE_IMAGE");
    let profile_path =
        std::env::var("PCX_PLAYER_RESOURCE_PROFILE").expect("PCX_PLAYER_RESOURCE_PROFILE");
    let image = std::fs::read(&image_path).unwrap();
    let profile = std::fs::read(&profile_path).unwrap();
    let rules = crate::player_resources::Rules::discover(
        &image,
        crate::scripts::AssetIdentity {
            source: image_path,
            edition: "explicit selected source copy".into(),
            version: "caller selected; image digest retained".into(),
        },
        &[(profile_path.as_str(), &profile)],
        crate::neutral_player::RuleInputBudget {
            image_bytes: 32 << 20,
            profile_bytes: 8 << 20,
            sections: 96,
        },
    )
    .unwrap();
    eprintln!(
        "resolved original food catalog={} image_sha256={:02x?}",
        root.display(),
        rules.image_sha256
    );
    marker_food_queries(ResolvedFoodCatalog(definitions), rules, false);
}
