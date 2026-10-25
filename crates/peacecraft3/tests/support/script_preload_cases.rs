use super::*;
use crate::binding::Reply;
use pcx_script::Host;
use std::collections::BTreeMap;

fn opened(files: &[(&str, &str)]) -> (App, Arc<pcx_assets::Resolver>) {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app.init_resource::<SelectedMapSources>();
    deliver(&mut app, MAP_OK, 7);
    let fixture = fixture(MAP_OK);
    let mut assets = BTreeMap::from([(
        "Source/Shared.j".into(),
        Reply::Bytes(fixture.common.clone()),
    )]);
    for &(path, text) in files {
        assets.insert(path.into(), Reply::Bytes(text.as_bytes().to_vec()));
    }
    let reader = Arc::new(fixture.resolver_with_assets(assets));
    let requests = app.world().resource::<MapRequests>();
    let mut rules = pcx_play::preload::Rules::fixture();
    rules.image_sha256 = host_inputs(Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0))
        .rules
        .slots
        .image_sha256();
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .unwrap()
        .admit_preloader(rules, Arc::clone(&reader))
        .unwrap();
    (app, reader)
}
#[test]
fn preload_executes_helpers_with_own_globals_and_reuses_the_canonical_world() {
    let script = "globals\ninteger marker=91\nendglobals\nfunction warm takes string path returns nothing\ncall Preload(path)\nendfunction\nfunction WarmFiles takes nothing returns nothing\nlocal integer i=0\nloop\nexitwhen i==2\ncall warm(\"Art/A.blp\")\nset i=i+1\nendloop\ncall Preload(\"Missing.blp\")\nif marker==91 then\ncall SetPlayerName(Player(4),\"warmed\")\nendif\ncall PreloadEnd(12.5)\nendfunction\n";
    let (mut app, reader) = opened(&[("Warm.hot", script), ("Art/A.blp", "selected asset")]);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let before = session.global("marker");
    let clock = session.global("CLOCK").unwrap();
    let host = session.host().unwrap();
    let time = host
        .native("GetFloatGameState", std::slice::from_ref(&clock))
        .unwrap();
    host.native("Preloader", &[Value::Str("Warm.hot".into())])
        .unwrap();
    let player = host.native("Player", &[Value::Int(4)]).unwrap();
    assert_eq!(
        host.native("GetPlayerName", &[player]).unwrap(),
        Value::Str("warmed".into())
    );
    assert_eq!(host.native("GetFloatGameState", &[clock]).unwrap(), time);
    let report = host.preload_report().unwrap();
    assert_eq!(report.queued, 3);
    assert_eq!(report.loaded, 2);
    assert_eq!(report.loaded_bytes, 28);
    assert_eq!(report.missing, ["Missing.blp"]);
    assert_eq!(report.timeout, Some(12.5));
    assert_eq!(report.scripts, ["Warm.hot"]);
    assert_eq!(
        reader.resolve_bounded("art/a.blp", 14).unwrap().bytes,
        b"selected asset"
    );
    assert_eq!(session.global("marker"), before);
}
#[test]
fn direct_preloads_validate_before_mutation_and_refresh_uses_the_source_batch() {
    let (mut app, _) = opened(&[("A.blp", "a"), ("B.blp", "bb"), ("C.blp", "ccc")]);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap();
    for path in ["Missing.blp", "A.blp", "B.blp", "C.blp"] {
        host.native("Preload", &[Value::Str(path.into())]).unwrap();
    }
    let before = host.preload_report().unwrap().clone();
    for (name, args) in [
        ("Preload", vec![Value::Str("../escape".into())]),
        ("Preload", vec![Value::Null]),
        ("PreloadEnd", vec![Value::Real(f32::NAN)]),
        ("PreloadEnd", vec![Value::Real(-1.)]),
        ("PreloadRefresh", vec![Value::Int(1)]),
    ] {
        assert!(host.native(name, &args).is_err());
        assert_eq!(host.preload_report(), Some(&before));
    }
    host.native("PreloadRefresh", &[]).unwrap();
    assert_eq!(host.preload_report().unwrap().loaded, 2);
    assert_eq!(host.preload_report().unwrap().missing, ["Missing.blp"]);
    host.native("PreloadEndEx", &[]).unwrap();
    assert_eq!(host.preload_report().unwrap().loaded, 3);
    host.native("Preload", &[Value::Str("A.blp".into())])
        .unwrap();
    host.native("PreloadStart", &[]).unwrap();
    assert_eq!(host.preload_report().unwrap().timeout, Some(4.125));
}
#[test]
fn unsupported_extensions_missing_files_recursion_and_foreign_callbacks_stay_bounded() {
    let callback = "function predicate takes nothing returns boolean\nreturn true\nendfunction\nfunction WarmFiles takes nothing returns nothing\ncall Filter(function predicate)\nendfunction\n";
    let (mut app, _) = opened(&[
        ("Invalid.hot", "not jass"),
        ("Callback.hot", callback),
        (
            "Loop.hot",
            "function WarmFiles takes nothing returns nothing\ncall Preloader(\"Loop.hot\")\nendfunction\n",
        ),
        (
            "Ok.hot",
            "function WarmFiles takes nothing returns nothing\ncall PreloadEnd(0.)\nendfunction\n",
        ),
    ]);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap();
    host.native("Preloader", &[Value::Str("Ignored.j".into())])
        .unwrap();
    assert!(host.preload_report().unwrap().scripts.is_empty());
    host.native("Preloader", &[Value::Str("Absent.hot".into())])
        .unwrap();
    assert_eq!(host.preload_report().unwrap().missing, ["Absent.hot"]);
    assert!(
        host.native("Preloader", &[Value::Str("Invalid.hot".into())])
            .is_err()
    );
    let handles = host.objects().inner().handles();
    let error = host
        .native("Preloader", &[Value::Str("Callback.hot".into())])
        .unwrap_err();
    assert!(error.contains("cross-program callbacks"), "{error}");
    assert_eq!(host.objects().inner().handles(), handles);
    assert!(
        host.native("Preloader", &[Value::Str("Loop.hot".into())])
            .unwrap_err()
            .contains("recursion")
    );
    host.native("Preloader", &[Value::Str("Ok.hot".into())])
        .unwrap();
}

#[test]
fn source_refusal_does_not_admit_or_execute_a_foreign_common() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app.init_resource::<SelectedMapSources>();
    deliver(&mut app, MAP_OK, 7);
    let fixture = fixture(MAP_OK);
    let reader=Arc::new(fixture.resolver_with_assets(BTreeMap::from([
        ("Source/Shared.j".into(),Reply::Bytes(b"// foreign common".to_vec())),
        ("Bad.hot".into(),Reply::Bytes(b"function WarmFiles takes nothing returns nothing\ncall SetPlayerName(Player(4),\"wrong\")\nendfunction\n".to_vec())),
    ])));
    let mut rules = pcx_play::preload::Rules::fixture();
    let requests = app.world().resource::<MapRequests>();
    let expected = host_inputs(Arc::clone(&requests.bindings.as_ref().unwrap().catalog.0))
        .rules
        .slots
        .image_sha256();
    rules.image_sha256 = [0xff; 32];
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let host = session.host().unwrap();
    assert!(
        host.admit_preloader(rules.clone(), Arc::clone(&reader))
            .is_err()
    );
    assert!(host.preload_report().is_none());
    rules.image_sha256 = expected;
    host.admit_preloader(rules.clone(), Arc::clone(&reader))
        .unwrap();
    assert!(host.admit_preloader(rules, reader).is_err());
    let player = host.native("Player", &[Value::Int(4)]).unwrap();
    host.native(
        "SetPlayerName",
        &[player.clone(), Value::Str("before".into())],
    )
    .unwrap();
    let before = host
        .native("GetPlayerName", std::slice::from_ref(&player))
        .unwrap();
    let error = host
        .native("Preloader", &[Value::Str("Bad.hot".into())])
        .unwrap_err();
    assert!(error.contains("resolved common differs"), "{error}");
    assert_eq!(host.native("GetPlayerName", &[player]).unwrap(), before);
    assert_eq!(host.preload_report().unwrap().queued, 0);
}
