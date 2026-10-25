use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic:weather".into(),
        edition: "synthetic".into(),
        version: "fixture".into(),
    }
}

fn sheet() -> Vec<u8> {
    let mut fields = vec![
        ("effectID".to_string(), "Wx93".to_string()),
        ("name".into(), "Synthetic storm".into()),
        ("texDir".into(), "Textures\\Weather".into()),
        ("texFile".into(), "Synthetic".into()),
        ("AmbientSound".into(), "_".into()),
    ];
    for name in [
        "alphaMode",
        "useFog",
        "height",
        "angx",
        "angy",
        "emrate",
        "lifespan",
        "particles",
        "veloc",
        "accel",
        "var",
        "texr",
        "texc",
        "head",
        "tail",
        "taillen",
        "lati",
        "long",
        "midTime",
        "version",
    ] {
        fields.push((
            name.into(),
            match name {
                "useFog" | "head" | "tail" => "1",
                "midTime" => "0.375",
                "emrate" => "37.5",
                "particles" => "73",
                "lifespan" => "2.75",
                _ => "2",
            }
            .into(),
        ));
    }
    for stage in ["Start", "Mid", "End"] {
        for channel in ["red", "green", "blue", "alpha"] {
            fields.push((format!("{channel}{stage}"), "53".into()));
        }
        for field in ["scale", "hUV", "tUV"] {
            fields.push((format!("{field}{stage}"), "1.25".into()));
        }
    }
    let mut text = "ID;PWXL;N;E\n".to_string();
    for (i, (key, value)) in fields.into_iter().enumerate() {
        text += &format!("C;X{};Y1;K\"{key}\"\nC;X{};Y2;K\"{value}\"\n", i + 1, i + 1);
    }
    text += "E\n";
    text.replace('\n', "\r\n").into_bytes()
}

fn catalog() -> Catalog {
    Catalog::read(&sheet(), origin(), 16384, 100).unwrap()
}

#[test]
fn table_preserves_nonstock_values_and_refuses_bad_sources() {
    let table = catalog();
    let row = table.get(*b"Wx93").unwrap();
    assert_eq!(row.texture, "Textures\\Weather\\Synthetic");
    assert_eq!(
        (row.emission_rate, row.lifespan, row.particles),
        (37.5, 2.75, 73)
    );
    assert_eq!(row.colors, [[53; 4]; 3]);
    assert_eq!(row.mid_time, 0.375);
    assert!(table.get(*b"Nope").is_none());
    assert!(Catalog::read(&sheet(), origin(), 1, 100).is_err());
    assert!(Catalog::read(&sheet(), origin(), 16384, 1).is_err());
    let original = String::from_utf8(sheet()).unwrap();
    for changed in [
        original.replace("37.5", "NaN"),
        original.replace("Wx93", "five!"),
        original.replace("midTime", "unknown"),
        original.replace("0.375", "1.5"),
    ] {
        assert!(Catalog::read(changed.as_bytes(), origin(), 16384, 100).is_err());
    }
    let duplicate = original.replace("E\r\n", "C;X1;Y3;K\"Wx93\"\r\nE\r\n");
    assert!(Catalog::read(duplicate.as_bytes(), origin(), 16384, 100).is_err());
}

const COMMON: &str = "type agent extends handle
type timer extends agent
type rect extends agent
type weathereffect extends handle
type playercolor extends handle
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native Rect takes real a, real b, real c, real d returns rect
native RemoveRect takes rect which returns nothing
native SetRect takes rect which, real a, real b, real c, real d returns nothing
native MoveRectTo takes rect which, real x, real y returns nothing
native GetRectCenterX takes rect which returns real
native GetRectCenterY takes rect which returns real
native GetRectMinX takes rect which returns real
native GetRectMinY takes rect which returns real
native GetRectMaxX takes rect which returns real
native GetRectMaxY takes rect which returns real
native AddWeatherEffect takes rect where, integer effectID returns weathereffect
native EnableWeatherEffect takes weathereffect whichEffect, boolean enable returns nothing
native RemoveWeatherEffect takes weathereffect whichEffect returns nothing
globals
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
";

fn host(common: &str, handles: usize) -> ObjectHost {
    let declarations = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &declarations)]).unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner)
        .unwrap()
        .with_rectangles()
        .unwrap()
}

fn input(initial_enabled: bool) -> Input {
    Input {
        catalog: catalog(),
        initial_enabled,
        world_bounds: [-50., -60., 70., 80.],
    }
}

#[test]
fn shared_handles_copy_clip_and_enable_weather_without_retaining_the_rect() {
    let mut host = host(COMMON, 16).with_weather(input(false)).unwrap();
    let rect = host
        .native("Rect", &[-90., -30., 40., 120.].map(Value::Real))
        .unwrap();
    let effect = host
        .native(
            "AddWeatherEffect",
            &[rect.clone(), Value::Int(i32::from_be_bytes(*b"Wx93"))],
        )
        .unwrap();
    assert_ne!(effect, rect);
    host.native("RemoveRect", std::slice::from_ref(&rect))
        .unwrap();
    let (_, state) = host.weather_effects().next().unwrap();
    assert_eq!(state.bounds, [-50., -30., 40., 80.]);
    assert!(!state.enabled);
    host.native("EnableWeatherEffect", &[effect.clone(), Value::Bool(true)])
        .unwrap();
    assert!(host.weather_effects().next().unwrap().1.enabled);
    assert!(
        host.native("EnableWeatherEffect", &[effect.clone(), Value::Int(1)])
            .is_err()
    );
    assert!(
        host.native("EnableWeatherEffect", &[rect, Value::Bool(false)])
            .is_err()
    );
    host.native("RemoveWeatherEffect", std::slice::from_ref(&effect))
        .unwrap();
    assert_eq!(host.weather_effects().count(), 0);
    host.native("EnableWeatherEffect", &[effect, Value::Bool(true)])
        .unwrap();
    assert_eq!(host.weather_effects().count(), 0);
}

#[test]
fn admission_and_failed_calls_preserve_the_existing_world() {
    assert!(
        host(&COMMON.replace("boolean enable", "integer enable"), 16)
            .with_weather(input(false))
            .is_err()
    );
    let mut host = host(COMMON, 2).with_weather(input(true)).unwrap();
    let rect = host
        .native("Rect", &[0., 0., 5., 7.].map(Value::Real))
        .unwrap();
    let args = [rect.clone(), Value::Int(i32::from_be_bytes(*b"Wx93"))];
    assert!(
        host.native("AddWeatherEffect", &[rect, Value::Int(7)])
            .is_err()
    );
    assert_eq!(host.weather_effects().count(), 0);
    host.native("AddWeatherEffect", &args).unwrap();
    assert!(host.weather_effects().next().unwrap().1.enabled);
    assert!(host.native("AddWeatherEffect", &args).is_err());
    assert_eq!(host.weather_effects().count(), 1);
}

#[test]
fn constructor_default_is_decoded_and_joined_to_the_enable_member() {
    let mut init = [0xc7, 0x45, 0xdc, 0, 0, 0, 0];
    let publish = [0x8b, 0x45, 0xdc, 0x89, 0x47, 0x28];
    let mut enable = [0x39, 0x4c, 0x02, 0x28, 0x74, 0xd3, 0x89, 0x4c, 0x02, 0x28];
    assert_eq!(initial_value(&init, &publish, &enable), Some(false));
    init[3] = 1;
    assert_eq!(initial_value(&init, &publish, &enable), Some(true));
    enable[9] = 0x24;
    assert_eq!(initial_value(&init, &publish, &enable), None);
    for n in 0..init.len() {
        assert_eq!(initial_value(&init[..n], &publish, &enable), None);
    }
}

#[test]
#[ignore = "requires PCX_WEATHER_SLK and PCX_WAR3_EXE from the selected local install"]
fn selected_original_weather_definitions_and_constructor_decode() {
    let path = std::env::var("PCX_WEATHER_SLK").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let catalog = Catalog::read(
        &bytes,
        AssetIdentity {
            source: path,
            edition: "local selected install".into(),
            version: "supplied original table bytes".into(),
        },
        4 << 20,
        10000,
    )
    .unwrap();
    assert!(catalog.definitions().next().is_some());
    let image = std::fs::read(std::env::var("PCX_WAR3_EXE").unwrap()).unwrap();
    assert!(!initial_enabled(&image).unwrap());
}
