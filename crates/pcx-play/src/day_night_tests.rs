use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "v-test".into(),
        source: "synthetic:archive".into(),
    }
}
fn input<'a>(path: &'a str, bytes: &'a [u8]) -> ModelInput<'a> {
    ModelInput {
        script_path: path,
        asset: ScriptAsset {
            label: "synthetic:model-member",
            bytes,
        },
        origin: origin(),
    }
}
fn chunk(tag: &[u8; 4], payload: &[u8], out: &mut Vec<u8>) {
    out.extend(tag);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
}
fn model(value: f32) -> Vec<u8> {
    let mut d = b"MDLX".to_vec();
    chunk(b"VERS", &800u32.to_le_bytes(), &mut d);
    let mut light = vec![0u8; 144];
    light[4..8].copy_from_slice(&96u32.to_le_bytes());
    light[8..13].copy_from_slice(b"light");
    light[92..96].copy_from_slice(&(-1i32).to_le_bytes());
    light[100..104].copy_from_slice(&1u32.to_le_bytes());
    for (at, v) in [
        (104, 7.0f32),
        (108, 19.0),
        (112, value),
        (116, 0.5),
        (120, 0.75),
        (124, 2.5),
        (128, 0.625),
        (132, 0.875),
        (136, 1.25),
        (140, 3.5),
    ] {
        light[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    light.extend(b"KLAC");
    light.extend(2u32.to_le_bytes());
    light.extend(1u32.to_le_bytes());
    light.extend(u32::MAX.to_le_bytes());
    for (frame, values) in [(0u32, [value, 0.2, 0.3]), (173, [0.6, 0.7, 0.8])] {
        light.extend(frame.to_le_bytes());
        for v in values {
            light.extend(v.to_le_bytes());
        }
    }
    let size = light.len() as u32;
    light[..4].copy_from_slice(&size.to_le_bytes());
    chunk(b"LITE", &light, &mut d);
    chunk(b"PIVT", &[0; 12], &mut d);
    d
}
fn budget() -> Budget {
    Budget {
        models: 4,
        model_bytes: 4096,
        total_bytes: 8192,
        path_bytes: 4096,
    }
}
fn assets() -> Assets {
    Assets::read(
        &[
            input("terrain.mdl", &model(0.125)),
            input("units.mdl", &model(0.375)),
        ],
        budget(),
    )
    .unwrap()
}
#[test]
fn explicit_bindings_preserve_complete_models_fingerprints_lights_and_tracks() {
    for value in [0.125, 0.375, 1.75] {
        let bytes = model(value);
        let a = Assets::read(&[input("selected.mdl", &bytes)], budget()).unwrap();
        let m = &a.models()[0];
        assert_eq!(m.script_path(), "selected.mdl");
        assert_eq!(m.source_label(), "synthetic:model-member");
        assert_eq!(m.origin(), &origin());
        assert_eq!(m.bytes(), bytes);
        assert_eq!(m.sha256(), sha256(&bytes));
        let l = &m.lights()[0];
        assert_eq!(l.colour[0].to_bits(), value.to_bits());
        assert_eq!(l.intensity, 2.5);
        assert_eq!(l.ambient_colour, [0.625, 0.875, 1.25]);
        assert_eq!(l.tracks[0].vec3(0).unwrap(), [value, 0.2, 0.3]);
        assert_eq!(l.tracks[0].frame(1), Some(173));
    }
}
#[test]
fn complete_set_budget_preflight_happens_before_any_model_parse() {
    let bad = b"not MDX";
    let big = model(0.125);
    let mut b = budget();
    b.total_bytes = bad.len() + big.len() - 1;
    let e = Assets::read(&[input("a", bad), input("b", &big)], b).unwrap_err();
    assert!(matches!(e.problem, Problem::Budget));
    assert_eq!(e.source_context, "synthetic:model-member");
    let bytes = model(0.125);
    let fields = input("a", &bytes);
    let path_bytes = fields.script_path.len()
        + fields.asset.label.len()
        + fields.origin.source.len()
        + fields.origin.edition.len()
        + fields.origin.version.len();
    let exact = Budget {
        models: 1,
        model_bytes: bytes.len(),
        total_bytes: bytes.len(),
        path_bytes,
    };
    assert!(Assets::read(&[input("a", &bytes)], exact).is_ok());
    for which in 0..4 {
        let mut b = exact;
        match which {
            0 => b.models = 0,
            1 => b.model_bytes -= 1,
            2 => b.total_bytes -= 1,
            _ => b.path_bytes -= 1,
        }
        assert!(matches!(
            Assets::read(&[input("a", &bytes)], b).unwrap_err().problem,
            Problem::Budget
        ));
    }
}
#[test]
fn platform_layouts_are_validated_before_decode() {
    for which in 0..4 {
        let mut b = budget();
        match which {
            0 => b.models = usize::MAX,
            1 => b.model_bytes = usize::MAX,
            2 => b.total_bytes = usize::MAX,
            _ => b.path_bytes = usize::MAX,
        }
        assert!(matches!(
            Assets::read(&[], b).unwrap_err().problem,
            Problem::Budget
        ));
    }
}
#[test]
fn missing_identity_duplicates_and_malformed_models_report_source() {
    let bytes = model(0.125);
    assert!(matches!(
        Assets::read(&[input("a", &bytes), input("a", &bytes)], budget())
            .unwrap_err()
            .problem,
        Problem::Duplicate
    ));
    assert!(matches!(
        Assets::read(&[input("", &bytes)], budget())
            .unwrap_err()
            .problem,
        Problem::Identity
    ));
    let mut x = input("a", &bytes);
    x.origin.version.clear();
    assert!(Assets::read(&[x], budget()).is_err());
    let e = Assets::read(&[input("a", b"broken")], budget()).unwrap_err();
    assert!(matches!(e.problem, Problem::Decode(_)));
    assert!(e.to_string().contains("synthetic:model-member"));
    let mut no_light = b"MDLX".to_vec();
    chunk(b"VERS", &800u32.to_le_bytes(), &mut no_light);
    assert!(matches!(
        Assets::read(&[input("a", &no_light)], budget())
            .unwrap_err()
            .problem,
        Problem::MissingLights
    ));
    let mut repeated = bytes.clone();
    chunk(b"PIVT", &[0; 12], &mut repeated);
    assert!(matches!(
        Assets::read(&[input("a", &repeated)], budget())
            .unwrap_err()
            .problem,
        Problem::DuplicateChunk(_)
    ));
    for node_size in [0, 95, 97, u32::MAX - 3, u32::MAX] {
        let mut invalid_node = bytes.clone();
        invalid_node[28..32].copy_from_slice(&node_size.to_le_bytes());
        assert!(Assets::read(&[input("a", &invalid_node)], budget()).is_err());
    }
    let mut malformed = bytes.clone();
    malformed[16 + 8 + 100..16 + 8 + 104].copy_from_slice(&99u32.to_le_bytes());
    assert!(Assets::read(&[input("a", &malformed)], budget()).is_err());
}
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative SetDayNightModels takes string terrain, string units returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let e = EnumHost::checked_selected(
        p,
        "synthetic:lighting-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(e).unwrap()
}
fn set(h: &mut ObjectHost, terrain: &str, units: &str) -> Result<Value, String> {
    h.native(
        "SetDayNightModels",
        &[Value::Str(terrain.into()), Value::Str(units.into())],
    )
}
#[test]
fn selection_requires_admission_and_failed_pair_is_atomic() {
    let mut disabled = host(COMMON, "");
    assert!(set(&mut disabled, "terrain.mdl", "units.mdl").is_err());
    let mut h = disabled.with_day_night(assets()).unwrap();
    assert!(h.day_night_state().unwrap().selected().is_none());
    assert_eq!(
        set(&mut h, "terrain.mdl", "units.mdl").unwrap(),
        Value::Null
    );
    let pair = h.day_night_state().unwrap().selected().unwrap();
    assert_eq!(pair[0].lights()[0].colour[0], 0.125);
    assert_eq!(pair[1].lights()[0].colour[0], 0.375);
    assert!(
        set(&mut h, "units.mdl", "unbound.mdl")
            .unwrap_err()
            .contains("unbound.mdl")
    );
    let pair = h.day_night_state().unwrap().selected().unwrap();
    assert_eq!(pair[0].script_path(), "terrain.mdl");
    assert_eq!(pair[1].script_path(), "units.mdl");
    assert!(set(&mut h, "terrain.mdx", "units.mdl").is_err());
    assert!(set(&mut h, "TERRAIN.mdl", "units.mdl").is_err());
    assert_eq!(h.inner().handles(), 0);
    set(&mut h, "units.mdl", "terrain.mdl").unwrap();
    assert_eq!(
        h.day_night_state().unwrap().selected().unwrap()[0].script_path(),
        "units.mdl"
    );
    assert!(h.with_day_night(assets()).is_err());
}
#[test]
fn bad_arguments_and_oversized_unknown_paths_leave_selection_unset() {
    let mut h = host(COMMON, "").with_day_night(assets()).unwrap();
    for args in [
        vec![],
        vec![Value::Null, Value::Null],
        vec![Value::Int(1), Value::Str("units.mdl".into())],
    ] {
        assert!(h.native("SetDayNightModels", &args).is_err());
    }
    assert!(
        set(&mut h, &"x".repeat(budget().path_bytes + 1), "units.mdl")
            .unwrap_err()
            .contains("policy exceeded")
    );
    assert!(set(&mut h, "", "units.mdl").is_err());
    assert!(h.day_night_state().unwrap().selected().is_none());
}

#[test]
fn replacement_models_commit_only_after_both_paths_resolve() {
    let mut h = host(COMMON, "").with_day_night(assets()).unwrap();
    set(&mut h, "terrain.mdl", "units.mdl").unwrap();
    let before = h
        .day_night_state()
        .unwrap()
        .selected()
        .unwrap()
        .map(|model| model.sha256());
    assert!(
        h.apply_day_night_assets(assets(), ["terrain.mdl", "missing.mdl"])
            .is_err()
    );
    assert_eq!(
        h.day_night_state()
            .unwrap()
            .selected()
            .unwrap()
            .map(|model| model.sha256()),
        before
    );
    h.apply_day_night_assets(assets(), ["units.mdl", "terrain.mdl"])
        .unwrap();
    assert_eq!(
        h.day_night_state().unwrap().selected().unwrap()[0].script_path(),
        "units.mdl"
    );
    assert_eq!(h.inner().handles(), 0);
}
#[test]
fn exact_native_contracts_and_script_overrides_are_required() {
    for common in [
        COMMON.replace(
            "native SetDayNightModels",
            "constant native SetDayNightModels",
        ),
        COMMON.replace(
            "string units returns nothing",
            "string units returns integer",
        ),
        COMMON.replace(
            "string terrain, string units",
            "integer terrain, string units",
        ),
    ] {
        assert!(host(&common, "").with_day_night(assets()).is_err());
    }
    assert!(
        host(
            COMMON,
            "function SetDayNightModels takes string a, string b returns nothing\nendfunction\n"
        )
        .with_day_night(assets())
        .is_err()
    );
}
#[test]
fn actual_owned_vm_selects_loaded_models_without_new_handles_or_fallbacks() {
    let mut vm=host(COMMON,"function main takes nothing returns nothing\ncall SetDayNightModels(\"terrain.mdl\",\"units.mdl\")\nendfunction\n").with_day_night(assets()).unwrap().into_vm();
    vm.init_globals().unwrap();
    let handles = vm.host().inner().handles();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
    assert_eq!(
        vm.host().day_night_state().unwrap().selected().unwrap()[1].script_path(),
        "units.mdl"
    );
}
/// `model(0.125)` with a fixed node rotation key and a 0..173 day sequence.
fn sun_model(rotation: Option<[f32; 4]>) -> Vec<u8> {
    let mut d = model(0.125);
    // MDLX, VERS chunk, then the LITE chunk: payload at 24, node ends at 124.
    if let Some(q) = rotation {
        let mut track = b"KGRT".to_vec();
        for v in [1u32, 0, u32::MAX, 0] {
            track.extend(v.to_le_bytes());
        }
        for v in q {
            track.extend(v.to_le_bytes());
        }
        let grow = track.len() as u32;
        d.splice(124..124, track);
        for at in [20, 24, 28] {
            let size = u32::from_le_bytes(d[at..at + 4].try_into().unwrap()) + grow;
            d[at..at + 4].copy_from_slice(&size.to_le_bytes());
        }
    }
    let mut sequence = vec![0u8; 132];
    sequence[..3].copy_from_slice(b"Day");
    sequence[84..88].copy_from_slice(&173u32.to_le_bytes());
    chunk(b"SEQS", &sequence, &mut d);
    d
}
#[test]
fn sun_samples_colour_tracks_along_the_day_sequence_and_rotates_down_axis() {
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let bytes = sun_model(Some([half, 0.0, 0.0, half]));
    let a = Assets::read(&[input("units.mdl", &bytes)], budget()).unwrap();
    let m = &a.models()[0];
    let dawn = m.sun(0.0).unwrap();
    assert_eq!(dawn.colour, [0.125, 0.2, 0.3]);
    assert_eq!(m.sun(1.0).unwrap().colour, [0.6, 0.7, 0.8]);
    assert_eq!(dawn.intensity, 2.5);
    assert_eq!(dawn.ambient_colour, [0.625, 0.875, 1.25]);
    assert_eq!(dawn.ambient_intensity, 3.5);
    // 90 degrees about +X turns -Z onto +Y.
    let [x, y, z] = dawn.direction;
    assert!(x.abs() < 1e-6 && (y - 1.0).abs() < 1e-6 && z.abs() < 1e-6);
    for fraction in [-0.1, 1.1, f32::NAN] {
        assert!(m.sun(fraction).is_err());
    }
    let unrotated = sun_model(None);
    let a = Assets::read(&[input("units.mdl", &unrotated)], budget()).unwrap();
    assert!(
        a.models()[0]
            .sun(0.5)
            .unwrap_err()
            .contains("missing fixed rotation key")
    );
}
