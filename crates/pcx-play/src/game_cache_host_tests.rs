use super::*;
use crate::{
    game_cache::tests::{Memory, budget},
    playercolor_host::{EnumHost, Limits},
};
use pcx_script::{Host, Prelude};
use std::sync::atomic::Ordering;

const COMMON: &str = "type agent extends handle
type timer extends agent
type gamecache extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer n returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native InitGameCache takes string file returns gamecache
native SaveGameCache takes gamecache c returns boolean
native ReloadGameCachesFromDisk takes nothing returns boolean
native FlushGameCache takes gamecache c returns nothing
native FlushStoredMission takes gamecache c, string m returns nothing
native StoreInteger takes gamecache c, string m, string k, integer v returns nothing
native StoreReal takes gamecache c, string m, string k, real v returns nothing
native StoreBoolean takes gamecache c, string m, string k, boolean v returns nothing
native StoreString takes gamecache c, string m, string k, string v returns boolean
native GetStoredInteger takes gamecache c, string m, string k returns integer
native GetStoredReal takes gamecache c, string m, string k returns real
native GetStoredBoolean takes gamecache c, string m, string k returns boolean
native GetStoredString takes gamecache c, string m, string k returns string
native HaveStoredInteger takes gamecache c, string m, string k returns boolean
native HaveStoredReal takes gamecache c, string m, string k returns boolean
native HaveStoredBoolean takes gamecache c, string m, string k returns boolean
native HaveStoredString takes gamecache c, string m, string k returns boolean
native FlushStoredInteger takes gamecache c, string m, string k returns nothing
native FlushStoredReal takes gamecache c, string m, string k returns nothing
native FlushStoredBoolean takes gamecache c, string m, string k returns nothing
native FlushStoredString takes gamecache c, string m, string k returns nothing
globals
constant playercolor COLOR=ConvertPlayerColor(73)
endglobals
";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:cache",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 32,
                admission_steps: 1024,
                expression_depth: 16,
                expression_work: 1024,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
fn cache(h: &mut ObjectHost, file: &str) -> Value {
    h.native("InitGameCache", &[Value::Str(file.into())])
        .unwrap()
}
fn args(c: &Value) -> Vec<Value> {
    vec![
        c.clone(),
        Value::Str("mission".into()),
        Value::Str("key".into()),
    ]
}

#[test]
fn owned_vm_stores_independent_types_and_reopens_the_saved_profile_in_a_new_host() {
    let store = Arc::new(Memory::default());
    let mut h = host(
        COMMON,
        "globals\ngamecache C=null\nendglobals\nfunction setup takes nothing returns boolean\nset C=InitGameCache(\"Cache.w3v\")\ncall StoreInteger(C,\"Mission\",\"Key\",-7)\ncall StoreReal(C,\"Mission\",\"Key\",2.5)\ncall StoreBoolean(C,\"Mission\",\"Key\",false)\nreturn StoreString(C,\"Mission\",\"Key\",\"héllo\")\nendfunction\n",
    );
    h.admit_game_caches(store.clone(), budget()).unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    assert_eq!(vm.call("setup", &[]).unwrap(), Value::Bool(true));
    let c = vm.global("C");
    let h = vm.host();
    let a = args(&c);
    assert_eq!(h.native("GetStoredInteger", &a).unwrap(), Value::Int(-7));
    assert_eq!(h.native("GetStoredReal", &a).unwrap(), Value::Real(2.5));
    assert_eq!(
        h.native("HaveStoredBoolean", &a).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("GetStoredBoolean", &a).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        h.native("GetStoredString", &a).unwrap(),
        Value::Str("héllo".into())
    );
    let alias = cache(h, "CACHE.W3V");
    assert_ne!(alias, c);
    assert_eq!(
        h.native("GetStoredInteger", &args(&alias)).unwrap(),
        Value::Int(-7)
    );
    let second = cache(h, "another");
    let mut put = args(&second);
    put.push(Value::Int(23));
    h.native("StoreInteger", &put).unwrap();
    assert_eq!(h.native("SaveGameCache", &[c]).unwrap(), Value::Bool(true));
    let mut next = host(COMMON, "");
    next.admit_game_caches(store, budget()).unwrap();
    let c = cache(&mut next, "cache.w3v");
    let other = cache(&mut next, "another");
    assert_eq!(
        next.native("GetStoredInteger", &args(&c)).unwrap(),
        Value::Int(-7)
    );
    assert_eq!(
        next.native("GetStoredInteger", &args(&other)).unwrap(),
        Value::Int(23)
    );
    let empty = cache(&mut next, "empty");
    assert_eq!(
        next.native("GetStoredString", &args(&empty)).unwrap(),
        Value::Null
    );
}

#[test]
fn flush_reload_and_failed_persistence_preserve_handle_and_save_boundaries() {
    let store = Arc::new(Memory::default());
    let mut h = host(COMMON, "");
    h.admit_game_caches(store.clone(), budget()).unwrap();
    let c = cache(&mut h, "file");
    let alias = cache(&mut h, "file");
    let mut put = args(&c);
    put.push(Value::Real(-0.0));
    h.native("StoreReal", &put).unwrap();
    assert_eq!(
        h.native("SaveGameCache", std::slice::from_ref(&c)).unwrap(),
        Value::Bool(true)
    );
    let saved = store.bytes.lock().unwrap().clone();
    h.native(
        "FlushStoredMission",
        &[c.clone(), Value::Str("mission".into())],
    )
    .unwrap();
    store.fail_read.store(true, Ordering::Relaxed);
    assert_eq!(
        h.native("ReloadGameCachesFromDisk", &[]).unwrap(),
        Value::Bool(false)
    );
    assert!(h.game_cache_storage_error().is_some());
    assert_eq!(
        h.native("HaveStoredReal", &args(&alias)).unwrap(),
        Value::Bool(false)
    );
    store.fail_read.store(false, Ordering::Relaxed);
    assert_eq!(
        h.native("ReloadGameCachesFromDisk", &[]).unwrap(),
        Value::Bool(true)
    );
    assert!(h.game_cache_storage_error().is_none());
    let Value::Real(v) = h.native("GetStoredReal", &args(&alias)).unwrap() else {
        panic!("real")
    };
    assert_eq!(v.to_bits(), (-0.0f32).to_bits());
    h.native("FlushGameCache", std::slice::from_ref(&c))
        .unwrap();
    assert!(h.native("SaveGameCache", &[c]).is_err());
    assert_eq!(
        h.native("HaveStoredReal", &args(&alias)).unwrap(),
        Value::Bool(false)
    );
    store.fail_write.store(true, Ordering::Relaxed);
    assert_eq!(
        h.native("SaveGameCache", std::slice::from_ref(&alias))
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(*store.bytes.lock().unwrap(), saved);
    let mut put = args(&alias);
    put.push(Value::Int(5));
    h.native("StoreInteger", &put).unwrap();
    store.fail_write.store(false, Ordering::Relaxed);
    assert_eq!(
        h.native("SaveGameCache", &[alias]).unwrap(),
        Value::Bool(true)
    );
    assert!(h.game_cache_storage_error().is_none());
    assert_eq!(
        h.native("SaveGameCache", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn cache_admission_and_mutation_refuse_bad_sources_formats_budgets_and_identities() {
    let store = Arc::new(Memory::default());
    let mut h = host(COMMON, "");
    assert!(
        h.native("InitGameCache", &[Value::Str("f".into())])
            .is_err()
    );
    for common in [
        COMMON.replace(
            "native StoreInteger takes",
            "constant native StoreInteger takes",
        ),
        COMMON.replace(
            "type gamecache extends agent",
            "type gamecache extends handle",
        ),
    ] {
        assert!(
            host(&common, "")
                .admit_game_caches(store.clone(), budget())
                .is_err()
        );
    }
    let decl = "native InitGameCache takes string file returns gamecache";
    assert!(
        host(&COMMON.replace(decl, ""), decl)
            .admit_game_caches(store.clone(), budget())
            .is_err()
    );
    *store.bytes.lock().unwrap() = Some(b"unsupported original or corrupt profile".to_vec());
    assert!(h.admit_game_caches(store.clone(), budget()).is_err());
    assert!(h.game_caches.is_none());
    *store.bytes.lock().unwrap() = None;
    h.admit_game_caches(store, budget()).unwrap();
    let c = cache(&mut h, "f");
    let before = h.inner().handles();
    assert!(
        h.native("InitGameCache", &[Value::Str("x".repeat(129))])
            .is_err()
    );
    assert_eq!(h.inner().handles(), before);
    assert_eq!(
        h.native("InitGameCache", &[Value::Str(String::new())])
            .unwrap(),
        Value::Null
    );
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(h.native("SaveGameCache", &[timer]).is_err());
    let mut bad = args(&c);
    bad.push(Value::Str("x".repeat(257)));
    assert!(h.native("StoreString", &bad).is_err());
    assert_eq!(
        h.native("HaveStoredString", &args(&c)).unwrap(),
        Value::Bool(false)
    );
    assert!(h.native("StoreUnit", &[]).is_err());
    assert!(h.native("SyncStoredInteger", &args(&c)).is_err());
}
