use super::*;

const MUSIC: &str =
    "SliderMinValue 10,\nSliderMaxValue 90,\nSliderInitialValue 70,\nSliderStepSize 6,";
const SOUND: &str =
    "SliderMinValue -50,\nSliderMaxValue 150,\nSliderInitialValue 0,\nSliderStepSize 13,";

fn fdf(music: &str, sound: &str) -> Fdf {
    Fdf::parse(&format!(
        "Frame \"GLUECHECKBOX\" \"MusicCheckBox\" {{\n}}\n\
         Frame \"CHECKBOX\" \"SoundCheckBox\" {{\n}}\n\
         Frame \"SLIDER\" \"MusicVolumeSlider\" {{\n{music}\n}}\n\
         Frame \"SLIDER\" \"SoundVolumeSlider\" {{\n{sound}\n}}\n"
    ))
    .unwrap()
}

fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic-edition".into(),
        version: "synthetic-client-version".into(),
        source: "synthetic-selected-image".into(),
    }
}

fn fields() -> [(&'static str, &'static [u8]); 4] {
    [
        ("music", b"1"),
        ("sfx", b"0"),
        ("musicvolume", b"31"),
        ("sfxvolume", b"50"),
    ]
}

fn candidate(fdf: &Fdf, fields: &[(&str, &[u8])]) -> Result<Seed, String> {
    prepare_with(&origin(), [0x5a; 32], fdf, |name| {
        fields.iter().find(|(key, _)| *key == name).map(|(_, v)| *v)
    })
}

fn failure(fdf: &Fdf, fields: &[(&str, &[u8])]) -> String {
    candidate(fdf, fields).err().expect("candidate must refuse")
}

#[test]
fn nonstandard_ranges_preserve_units_without_snapping_or_widget_initials() {
    let _public_entry = prepare;
    let seed = candidate(&fdf(MUSIC, SOUND), &fields()).unwrap();
    assert_eq!(seed.values.len(), 2);
    assert_eq!(seed.toggles.len(), 2);
    assert_eq!(seed.ranges.len(), 2);
    assert_eq!(seed.values["MusicVolumeSlider"], 31.0);
    assert_eq!(seed.values["SoundVolumeSlider"], 50.0);
    assert_eq!(seed.audio.music_gain, 21.0 / 80.0);
    assert_eq!(seed.audio.effects_gain, 0.5);
    assert!(seed.audio.music_enabled);
    assert!(!seed.audio.effects_enabled);
    assert!(seed.toggles["MusicCheckBox"]);
    assert!(!seed.toggles["SoundCheckBox"]);
    let range = seed.ranges["MusicVolumeSlider"];
    assert_eq!((range.min, range.max, range.step), (10.0, 90.0, 6.0));
    seed.audio.validate().unwrap();
}

#[test]
fn complete_candidate_already_contains_both_first_voice_gains_and_flags() {
    for music in [b"0".as_slice(), b"1"] {
        for sfx in [b"0".as_slice(), b"1"] {
            let mut input = fields();
            input[0].1 = music;
            input[1].1 = sfx;
            let seed = candidate(&fdf(MUSIC, SOUND), &input).unwrap();
            assert_eq!(seed.audio.music_enabled, music == b"1");
            assert_eq!(seed.audio.effects_enabled, sfx == b"1");
            assert_eq!(
                seed.audio.effective_music(),
                if music == b"1" { 21.0 / 80.0 } else { 0.0 }
            );
            assert_eq!(
                seed.audio.effective_effects(),
                if sfx == b"1" { 0.5 } else { 0.0 }
            );
        }
    }
}

#[test]
fn every_required_field_and_empty_fallback_refuses_a_complete_candidate() {
    let fdf = fdf(MUSIC, SOUND);
    for index in 0..fields().len() {
        let input = fields();
        let missing: Vec<_> = input
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index)
            .map(|(_, entry)| *entry)
            .collect();
        assert!(failure(&fdf, &missing).contains(input[index].0));
        let mut empty = input;
        empty[index].1 = b"";
        assert!(failure(&fdf, &empty).contains(input[index].0));
    }
    assert!(failure(&fdf, &[]).contains("missing source fallback"));
}

#[test]
fn malformed_overflow_and_non_utf8_scalars_never_use_another_value() {
    let fdf = fdf(MUSIC, SOUND);
    for bytes in [
        b"31x".as_slice(),
        b" 31",
        b"31 ",
        b"31\n",
        b"31.0",
        b"0x1f",
        b"2147483648",
        b"-2147483649",
        b"--31",
        b"31\0",
        b"\xff",
    ] {
        let mut input = fields();
        input[2].1 = bytes;
        let error = failure(&fdf, &input);
        assert!(error.contains("musicvolume"));
        assert!(error.contains("synthetic-edition/synthetic-client-version"));
        assert!(error.contains("synthetic-selected-image"));
    }
}

#[test]
fn toggles_require_the_supported_zero_one_profile() {
    for index in [0, 1] {
        for bytes in [b"2".as_slice(), b"-1", b"true", b"false", b"1.0"] {
            let mut input = fields();
            input[index].1 = bytes;
            assert!(failure(&fdf(MUSIC, SOUND), &input).contains(input[index].0));
        }
    }
}

#[test]
fn outside_range_values_refuse_instead_of_clamping() {
    for (index, bytes) in [(2, b"9".as_slice()), (2, b"91"), (3, b"-51"), (3, b"151")] {
        let mut input = fields();
        input[index].1 = bytes;
        assert!(failure(&fdf(MUSIC, SOUND), &input).contains("authored slider range"));
    }
}

#[test]
fn exact_endpoints_and_negative_source_units_are_supported() {
    for (music, sound, sound_units, gains) in [
        (b"10".as_slice(), b"-50".as_slice(), -50.0, (0.0, 0.0)),
        (b"90".as_slice(), b"150".as_slice(), 150.0, (1.0, 1.0)),
    ] {
        let mut input = fields();
        input[2].1 = music;
        input[3].1 = sound;
        let seed = candidate(&fdf(MUSIC, SOUND), &input).unwrap();
        assert_eq!((seed.audio.music_gain, seed.audio.effects_gain), gains);
        assert_eq!(seed.values["SoundVolumeSlider"], sound_units);
    }
}

#[test]
fn integer_units_must_survive_the_existing_f32_slider_representation_exactly() {
    let wide =
        "SliderMinValue 0,\nSliderMaxValue 20000000,\nSliderInitialValue 0,\nSliderStepSize 1,";
    let mut input = fields();
    input[2].1 = b"16777217";
    assert!(failure(&fdf(wide, SOUND), &input).contains("represented exactly"));
    input[2].1 = b"16777216";
    assert_eq!(
        candidate(&fdf(wide, SOUND), &input).unwrap().values["MusicVolumeSlider"],
        16777216.0
    );
}

#[test]
fn every_authored_slider_property_is_required_even_when_initial_is_not_seeded() {
    for property in MUSIC.lines() {
        let missing = MUSIC.replace(property, "");
        assert!(failure(&fdf(&missing, SOUND), &fields()).contains("musicvolume"));
    }
    for (from, to) in [
        ("SliderStepSize 6", "SliderStepSize 0"),
        ("SliderStepSize 6", "SliderStepSize -2"),
        ("SliderMinValue 10", "SliderMinValue 90"),
        ("SliderMinValue 10", "SliderMinValue 100"),
        ("SliderInitialValue 70", "SliderInitialValue 91"),
        ("SliderStepSize 6", "SliderStepSize NaN"),
        ("SliderMaxValue 90", "SliderMaxValue inf"),
        ("SliderMinValue 10", "SliderMinValue bad"),
    ] {
        assert!(failure(&fdf(&MUSIC.replace(from, to), SOUND), &fields()).contains("musicvolume"));
    }
    let overflow =
        "SliderMinValue -3e38,\nSliderMaxValue 3e38,\nSliderInitialValue 0,\nSliderStepSize 1,";
    assert!(failure(&fdf(overflow, SOUND), &fields()).contains("slider range"));
}

#[test]
fn missing_frames_and_wrong_widget_types_refuse() {
    assert!(failure(&Fdf::default(), &fields()).contains("MusicCheckBox"));
    let mut wrong = fdf(MUSIC, SOUND);
    wrong
        .parse_into("Frame \"TEXT\" \"MusicCheckBox\" {\n}\n")
        .unwrap();
    assert!(failure(&wrong, &fields()).contains("supported checkbox"));
    let mut wrong = fdf(MUSIC, SOUND);
    wrong
        .parse_into("Frame \"CHECKBOX\" \"SoundVolumeSlider\" {\n}\n")
        .unwrap();
    assert!(failure(&wrong, &fields()).contains("supported slider"));
}

#[test]
fn inherited_slider_ranges_use_the_same_house_widget_resolution() {
    let mut inherited = fdf(MUSIC, SOUND);
    inherited
        .parse_into(&format!(
            "Frame \"SLIDER\" \"SyntheticRange\" {{\n{MUSIC}\n}}\n\
         Frame \"SLIDER\" \"MusicVolumeSlider\" INHERITS \"SyntheticRange\" {{\n}}\n"
        ))
        .unwrap();
    let seed = candidate(&inherited, &fields()).unwrap();
    assert_eq!(seed.values["MusicVolumeSlider"], 31.0);
    assert_eq!(seed.audio.music_gain, 21.0 / 80.0);
}

#[test]
fn receipt_and_candidate_are_owned_independently_of_input_storage() {
    let seed = {
        let mut identity = origin();
        let source = b"31".to_vec();
        let seed = prepare_with(&identity, [0x3c; 32], &fdf(MUSIC, SOUND), |key| match key {
            "music" => Some(b"1"),
            "sfx" => Some(b"0"),
            "musicvolume" => Some(source.as_slice()),
            "sfxvolume" => Some(b"50"),
            _ => None,
        })
        .unwrap();
        identity.source.clear();
        seed
    };
    assert_eq!(seed.origin, origin());
    assert_eq!(seed.image_sha256, [0x3c; 32]);
    assert_eq!(seed.values["MusicVolumeSlider"], 31.0);
}
