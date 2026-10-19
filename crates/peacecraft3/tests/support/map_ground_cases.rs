use crate::{binding, browser_configuration_cases as configuration};
use bevy::{asset::AssetPlugin, prelude::*};
use pcx_gfx::{GroundMaterial, GroundMemory};
use peacecraft3::{
    browser_configuration::{ConfigurationInbox, Poll, SelectedMapSources},
    map_ground::{GroundPublication, GroundSupply, MapGroundError},
    map_requests::MapRequests,
};
use std::{collections::BTreeMap, sync::Arc};

fn ground_fixture() -> binding::Fixture {
    let mut f = configuration::fixture(235);
    let terrain = &mut f
        .files
        .iter_mut()
        .find(|(n, _)| *n == "war3map.w3e")
        .unwrap()
        .1;
    terrain[13..17].copy_from_slice(&1u32.to_le_bytes());
    terrain.splice(17..17, *b"Qgrd");
    f.rebuild();
    f
}

use crate::ground_fixtures::assets as ground_assets;

fn ground_app(inbox: &ConfigurationInbox) -> App {
    let mut app = configuration::app(inbox);
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    peacecraft3::map_ground::install(&mut app);
    assert!(
        app.world()
            .contains_resource::<peacecraft3::map_ground::NoOcclusionPass>()
    );
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .insert_resource(GroundSupply {
            revision: 73,
            read_limit: Some(4096),
            memory: Some(GroundMemory {
                retained_bytes: 64,
                transient_bytes: 128,
            }),
        });
    app
}

fn enqueue(
    inbox: &ConfigurationInbox,
    f: &binding::Fixture,
    assets: BTreeMap<String, binding::Reply>,
) -> u32 {
    let mut request = configuration::request(f, f.resolver_with_assets(assets));
    request.limits.reader.max_listed_paths = 7;
    inbox.enqueue(request).unwrap()
}

#[test]
fn delivered_ground_uses_the_selected_reader_and_keeps_its_draw_on_refusal() {
    let inbox = ConfigurationInbox::default();
    let mut app = ground_app(&inbox);
    let f = ground_fixture();
    let id = enqueue(&inbox, &f, ground_assets());
    app.update();
    let Poll::Completed(outcome) = inbox.poll(id).unwrap() else {
        panic!("pending")
    };
    assert!(outcome.result.is_ok(), "{:?}", outcome.result);
    let published = app.world().resource::<GroundPublication>();
    assert!(published.last_error.is_none(), "{:?}", published.last_error);
    assert_eq!(published.receipts.len(), 2);
    let expected = ground_assets()
        .into_iter()
        .map(|(path, value)| (pcx_assets::AssetPath::new(&path).unwrap(), value))
        .collect::<std::collections::BTreeMap<_, _>>();
    for item in &published.receipts {
        assert_eq!(item.provenance.source, f.catalog_infos[0]);
        let binding::Reply::Bytes(bytes) = expected.get(&item.path).unwrap() else {
            panic!("not bytes")
        };
        assert_eq!(item.hash, pcx_mpq::fnv1a64(bytes));
        assert_eq!(item.provenance.byte_length, bytes.len() as u64);
    }
    let receipts = published.receipts.clone();
    let entity = published.entity.unwrap();
    let context = Arc::clone(published.context.as_ref().unwrap());
    assert!(Arc::ptr_eq(
        &context,
        &app.world()
            .resource::<MapRequests>()
            .admission
            .shared_context()
            .unwrap()
    ));
    assert!(app.world().get::<Mesh3d>(entity).is_some());
    let material = &app
        .world()
        .get::<MeshMaterial3d<GroundMaterial>>(entity)
        .unwrap()
        .0;
    let texture = &app
        .world()
        .resource::<Assets<GroundMaterial>>()
        .get(material)
        .unwrap()
        .texture;
    assert_eq!(
        app.world()
            .resource::<Assets<Image>>()
            .get(texture)
            .unwrap()
            .data
            .as_ref()
            .unwrap()
            .len(),
        64
    );

    let mut missing = ground_assets();
    missing.remove("Synthetic\\Ground.blp");
    let id = enqueue(&inbox, &f, missing);
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let published = app.world().resource::<GroundPublication>();
    assert!(matches!(
        published.last_error,
        Some(MapGroundError::Read { .. })
    ));
    assert_eq!(published.receipts, receipts);
    assert_eq!(published.entity, Some(entity));
    assert!(Arc::ptr_eq(published.context.as_ref().unwrap(), &context));
    assert!(!Arc::ptr_eq(
        &context,
        &app.world()
            .resource::<MapRequests>()
            .admission
            .shared_context()
            .unwrap()
    ));
    assert!(app.world().get::<Mesh3d>(entity).is_some());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
    assert_eq!(app.world().resource::<Assets<GroundMaterial>>().len(), 1);

    let id = enqueue(&inbox, &f, ground_assets());
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let next = app.world().resource::<GroundPublication>().entity.unwrap();
    assert_ne!(next, entity);
    assert!(app.world().get_entity(entity).is_err());
    assert!(app.world().get::<Mesh3d>(next).is_some());
}

#[test]
fn stale_ground_delivery_and_missing_inputs_preserve_the_mounted_source() {
    let inbox = ConfigurationInbox::default();
    let mut app = ground_app(&inbox);
    let f = ground_fixture();
    let id = enqueue(&inbox, &f, ground_assets());
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let old = app
        .world_mut()
        .remove_resource::<GroundPublication>()
        .unwrap();
    let entity = old.entity.unwrap();
    let context = old.context.unwrap();
    let mut publication = GroundPublication::default();
    publication.entity = Some(entity);
    publication.context = Some(Arc::clone(&context));
    app.insert_resource(publication);
    app.world_mut()
        .resource_mut::<MapRequests>()
        .last_outcome
        .as_mut()
        .unwrap()
        .binding_revision = Some(72);
    app.update();
    assert_eq!(
        app.world().resource::<GroundPublication>().entity,
        Some(entity)
    );
    assert!(
        app.world()
            .resource::<GroundPublication>()
            .last_error
            .is_none()
    );
    app.world_mut()
        .resource_mut::<MapRequests>()
        .last_outcome
        .as_mut()
        .unwrap()
        .binding_revision = Some(73);
    app.world_mut().resource_mut::<GroundSupply>().memory = None;
    app.update();
    assert!(matches!(
        app.world().resource::<GroundPublication>().last_error,
        Some(MapGroundError::MissingBudgets { binding: 73 })
    ));
    app.world_mut().resource_mut::<GroundSupply>().memory = Some(GroundMemory {
        retained_bytes: 64,
        transient_bytes: 128,
    });
    app.insert_resource(SelectedMapSources::default());
    app.update();
    let published = app.world().resource::<GroundPublication>();
    assert_eq!(
        published.last_error,
        Some(MapGroundError::MissingSourceReader)
    );
    assert_eq!(published.entity, Some(entity));
    assert!(Arc::ptr_eq(published.context.as_ref().unwrap(), &context));
    assert!(app.world().get::<Mesh3d>(entity).is_some());
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
}

#[test]
fn corrected_ground_budgets_retry_the_same_admitted_map() {
    for read_limit in [1, 4096] {
        let inbox = ConfigurationInbox::default();
        let mut app = ground_app(&inbox);
        app.world_mut().resource_mut::<GroundSupply>().read_limit = Some(read_limit);
        app.world_mut().resource_mut::<GroundSupply>().memory = Some(GroundMemory {
            retained_bytes: 63,
            transient_bytes: 128,
        });
        let f = ground_fixture();
        let id = enqueue(&inbox, &f, ground_assets());
        app.update();
        assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
        let publication = app.world().resource::<GroundPublication>();
        assert!(publication.entity.is_none());
        if read_limit == 1 {
            assert!(matches!(
                publication.last_error,
                Some(MapGroundError::Read { .. })
            ));
            app.world_mut().resource_mut::<GroundSupply>().read_limit = Some(4096);
            app.update();
        }
        assert!(matches!(
            app.world().resource::<GroundPublication>().last_error,
            Some(MapGroundError::Ground(
                pcx_gfx::GroundError::RetainedBytes { .. }
            ))
        ));
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        app.world_mut()
            .resource_mut::<GroundSupply>()
            .memory
            .as_mut()
            .unwrap()
            .retained_bytes = 64;
        app.update();
        let publication = app.world().resource::<GroundPublication>();
        assert!(
            publication.last_error.is_none(),
            "{:?}",
            publication.last_error
        );
        let entity = publication.entity.unwrap();
        let context = Arc::clone(publication.context.as_ref().unwrap());
        assert!(Arc::ptr_eq(
            &context,
            &app.world()
                .resource::<MapRequests>()
                .admission
                .shared_context()
                .unwrap()
        ));
        app.update();
        assert_eq!(
            app.world().resource::<GroundPublication>().entity,
            Some(entity)
        );
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        let memory = app.world_mut().resource_mut::<GroundSupply>().memory.take();
        app.update();
        assert!(matches!(
            app.world().resource::<GroundPublication>().last_error,
            Some(MapGroundError::MissingBudgets { .. })
        ));
        assert_eq!(
            app.world().resource::<GroundPublication>().entity,
            Some(entity)
        );
        app.world_mut().resource_mut::<GroundSupply>().memory = memory;
        app.update();
        assert!(
            app.world()
                .resource::<GroundPublication>()
                .last_error
                .is_none()
        );
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_ground_publication_gate() -> u32 {
    delivered_ground_uses_the_selected_reader_and_keeps_its_draw_on_refusal();
    stale_ground_delivery_and_missing_inputs_preserve_the_mounted_source();
    corrected_ground_budgets_retry_the_same_admitted_map();
    automatic_ground_budget_follows_delivery_and_preserves_refused_replacement();
    automatic_ground_budget_is_enforced_before_publication();
    5
}

fn automatic_app(inbox: &ConfigurationInbox, read_limit: u64, retained_bytes: usize) -> App {
    let mut app = configuration::app(inbox);
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    peacecraft3::map_ground::install_with_budget(
        &mut app,
        read_limit,
        GroundMemory {
            retained_bytes,
            transient_bytes: 128,
        },
    );
    app.init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>();
    app
}

#[test]
fn automatic_ground_budget_follows_delivery_and_preserves_refused_replacement() {
    let inbox = ConfigurationInbox::default();
    let mut app = automatic_app(&inbox, 4096, 64);
    app.update();
    assert_eq!(app.world().resource::<GroundSupply>().read_limit, None);
    let f = ground_fixture();
    let id = enqueue(&inbox, &f, ground_assets());
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let publication = app.world().resource::<GroundPublication>();
    assert!(
        publication.last_error.is_none(),
        "{:?}",
        publication.last_error
    );
    let first = publication.entity.unwrap();
    assert_eq!(app.world().resource::<GroundSupply>().revision, 73);
    app.update();
    assert_eq!(
        app.world().resource::<GroundPublication>().entity,
        Some(first)
    );
    assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);

    let mut request = configuration::request(&f, f.resolver_with_assets(ground_assets()));
    request.revision = 91;
    request.limits.reader.max_listed_paths = 7;
    let id = inbox.enqueue(request).unwrap();
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let publication = app.world().resource::<GroundPublication>();
    assert!(
        publication.last_error.is_none(),
        "{:?}",
        publication.last_error
    );
    let second = publication.entity.unwrap();
    let context = Arc::clone(publication.context.as_ref().unwrap());
    let receipts = publication.receipts.clone();
    assert_ne!(second, first);
    assert!(app.world().get_entity(first).is_err());
    assert_eq!(app.world().resource::<GroundSupply>().revision, 91);

    let mut missing = ground_assets();
    missing.remove("Synthetic\\Ground.blp");
    let id = enqueue(&inbox, &f, missing);
    app.update();
    assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
    let publication = app.world().resource::<GroundPublication>();
    assert!(matches!(
        publication.last_error,
        Some(MapGroundError::Read { .. })
    ));
    assert_eq!(publication.entity, Some(second));
    assert!(Arc::ptr_eq(publication.context.as_ref().unwrap(), &context));
    assert_eq!(publication.receipts, receipts);
    assert!(app.world().get::<Mesh3d>(second).is_some());
    assert_eq!(app.world().resource::<GroundSupply>().revision, 73);
}

#[test]
fn automatic_ground_budget_is_enforced_before_publication() {
    for (read_limit, retained_bytes) in [(1, 64), (4096, 63)] {
        let inbox = ConfigurationInbox::default();
        let mut app = automatic_app(&inbox, read_limit, retained_bytes);
        let f = ground_fixture();
        let id = enqueue(&inbox, &f, ground_assets());
        app.update();
        assert!(matches!(inbox.poll(id).unwrap(), Poll::Completed(o) if o.result.is_ok()));
        let publication = app.world().resource::<GroundPublication>();
        assert!(publication.entity.is_none());
        if read_limit == 1 {
            assert!(matches!(
                publication.last_error,
                Some(MapGroundError::Read { .. })
            ));
        } else {
            assert!(matches!(
                publication.last_error,
                Some(MapGroundError::Ground(
                    pcx_gfx::GroundError::RetainedBytes {
                        needed: 64,
                        limit: 63,
                        ..
                    }
                ))
            ));
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<GroundMaterial>>().len(), 0);
    }
}
