//! Parsed item catalog through delivered-map startup and the shared handle arena.
use super::*;
use crate::binding::Reply;
use pcx_play::playercolor_host::ItemBudget;
use pcx_script::{HandleId, Host};
use std::collections::BTreeMap;

const EXTRA: &str = "\
type item extends widget
native CreateItem takes integer typeId, real x, real y returns item
native SetItemDropID takes item whichItem, integer unitId returns nothing
native GetItemLevel takes item whichItem returns integer
native GetItemType takes item whichItem returns itemtype
";
const MAP: &str = "\
globals
timer before = null
item made = null
timer after = null
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set before = CreateTimer()
set made = CreateItem('ITEM',-3.5,0)
set after = CreateTimer()
endfunction
";
fn budget() -> ItemBudget {
    ItemBudget {
        max_records: 1,
        max_retained_fields: 4,
        max_modifications: 0,
    }
}
fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}
fn deliver(app: &mut App, revision: u64, items: Option<ItemBudget>, script: &str, extra: &str) {
    let mut map = fixture(script);
    map.common.extend_from_slice(extra.as_bytes());
    map.metadata = sylk(
        &["ID", "field", "slk", "type", "index", "category"],
        &[
            &[
                "STAT",
                "Score",
                "UnitData",
                "string",
                "-1",
                "synthetic unit",
            ],
            &[
                "ITXT",
                "Score",
                "ItemData",
                "string",
                "-1",
                "synthetic item",
            ],
        ],
    );
    map.rebuild();
    let item_data = sylk(&["itemID", "Score"], &[&["ITEM", "synthetic item field"]]);
    let resolver = map.resolver_with_assets(BTreeMap::from([(
        SlkId::ItemData.path().to_owned(),
        Reply::Bytes(item_data.clone()),
    )]));
    let tables = [SlkId::UnitData, SlkId::UnitMetaData, SlkId::ItemData];
    let mut input = map.input(revision);
    input.required_catalog_tables = &tables;
    let mut limits = map.limits();
    limits.reader.max_listed_paths += 1;
    limits.reader.max_asset_bytes = limits.reader.max_asset_bytes.max(item_data.len());
    limits.reader.max_aggregate_input_bytes += item_data.len();
    limits.max_resolver_input_bytes += item_data.len();
    let bound = bind_map(&resolver, input, limits).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.budgets.items = items;
    // Synthetic class table matching the declared `ConvertItemType(0)` conversion.
    inputs.item_classes = Some(pcx_play::item_class_table::ItemClassTable {
        origin: pcx_play::scripts::AssetIdentity {
            edition: "synthetic item fixture".into(),
            version: "synthetic".into(),
            source: "synthetic item fixture".into(),
        },
        image_sha256: [0; 32],
        ordinals: BTreeMap::from([("permanent".to_owned(), 0)]),
        default: 0,
    });
    // Synthetic fixture with no sale rows: this fixture exercises item creation, not sales.
    inputs.item_sales = Some(pcx_play::item_sales::ItemSaleTable::new(
        pcx_play::scripts::AssetIdentity {
            edition: "synthetic item fixture".into(),
            version: "synthetic".into(),
            source: "synthetic item fixture".into(),
        },
        BTreeMap::new(),
    ));
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic item map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
}
fn handle(session: &MapScriptSession<MapStartupHost>, name: &str) -> HandleId {
    let Some(Value::Handle(id)) = session.global(name) else {
        panic!("expected handle {name}")
    };
    id
}
#[test]
fn published_item_retains_the_parsed_catalog_source_and_executed_coordinates() {
    let mut app = app();
    deliver(&mut app, 1, Some(budget()), MAP, EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let catalog = Arc::clone(
        &app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .catalog
            .0,
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let source = session.context().unwrap().world.source.clone();
    let [before, made, after] = ["before", "made", "after"].map(|name| handle(&session, name));
    assert_eq!(made.token(), before.token() + 1);
    assert_eq!(after.token(), made.token() + 1);
    let host = session.host().unwrap();
    let record = host.objects().executed_item(made).unwrap();
    assert_eq!(record.definition, catalog.resolve(*b"ITEM").unwrap());
    assert_eq!(
        record.definition.kind,
        pcx_objects::editor::ObjectKind::Item
    );
    assert_eq!(
        record.definition.fields[b"ITXT"].value,
        Ok(pcx_sim::world::FieldValue::Cell(
            "synthetic item field".into()
        ))
    );
    assert_eq!(record.source, source);
    assert_eq!((record.x, record.y), (-3.5, 0.0));
    assert_eq!(host.objects().inner().kind(made).unwrap(), "item");
    assert!(host.objects().executed_item(before).is_err());
    assert!(host.native("DestroyTimer", &[Value::Handle(made)]).is_err());
    assert_eq!(host.objects().live_timers(), 2);
    for timer in [before, after] {
        host.native("DestroyTimer", &[Value::Handle(timer)])
            .unwrap();
    }
    assert_eq!(host.objects().live_timers(), 0);
    assert!(host.objects().executed_item(made).is_ok());
}
#[test]
fn item_refusals_preserve_the_published_session_and_valid_replacement_retires_it() {
    let mut app = app();
    deliver(&mut app, 1, Some(budget()), MAP, EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    let (witness, context, made) = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        (
            session.session_witness().unwrap(),
            Arc::clone(session.context().unwrap()),
            handle(session, "made"),
        )
    };
    let valid = budget();
    for (index, (items, script)) in [
        (None, MAP.to_owned()),
        (
            Some(ItemBudget {
                max_records: 0,
                ..valid
            }),
            MAP.to_owned(),
        ),
        (
            Some(ItemBudget {
                max_retained_fields: 0,
                ..valid
            }),
            MAP.to_owned(),
        ),
        (Some(valid), MAP.replace("'ITEM'", "'UNIT'")),
        (Some(valid), MAP.replace("'ITEM'", "'MISS'")),
    ]
    .into_iter()
    .enumerate()
    {
        deliver(&mut app, index as u64 + 2, items, &script, EXTRA);
        assert!(matches!(
            app.world().resource::<CampaignStartup>().last_error,
            Some(StartupError::Session(ScriptSessionError::Execution(_)))
        ));
        assert!(witness.upgrade().is_some());
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert!(Arc::ptr_eq(session.context().unwrap(), &context));
        let host = session.host().unwrap();
        assert_eq!(
            host.objects()
                .executed_item(made)
                .unwrap()
                .definition
                .rawcode,
            *b"ITEM"
        );
        assert_eq!(host.objects().live_timers(), 2);
    }
    deliver(
        &mut app,
        7,
        Some(valid),
        MAP_OK,
        "type item extends widget\n",
    );
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::Session(ScriptSessionError::Factory { .. }))
    ));
    assert!(witness.upgrade().is_some());
    deliver(&mut app, 8, Some(valid), MAP, EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    assert!(witness.upgrade().is_none());
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_item_factory_gate() -> u32 {
    published_item_retains_the_parsed_catalog_source_and_executed_coordinates();
    item_refusals_preserve_the_published_session_and_valid_replacement_retires_it();
    2
}
