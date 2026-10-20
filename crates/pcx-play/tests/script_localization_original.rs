//! Explicit local original-input gate; no original content is part of this fixture.
use pcx_play::{
    script_localization::{Bank, Budget, Rules},
    scripts::{AssetIdentity, ScriptAsset},
};
#[test]
#[ignore = "requires PCX_LOCALIZATION_{CLIENT,STRINGS,HOTKEYS} selected local assets"]
fn local_original_script_localization_reads_selected_defeat_labels() {
    let read = |name: &str| {
        let path = std::env::var(format!("PCX_LOCALIZATION_{name}")).unwrap();
        (path.clone(), std::fs::read(path).unwrap())
    };
    let (client, image) = read("CLIENT");
    let (strings, labels) = read("STRINGS");
    let (hotkeys, keys) = read("HOTKEYS");
    let origin = |source| AssetIdentity {
        source,
        edition: "selected local UI".into(),
        version: "selected install".into(),
    };
    let rules = Rules::reviewed_legacy(&image, origin(client)).unwrap();
    let bank = Bank::read(
        ScriptAsset {
            label: &strings,
            bytes: &labels,
        },
        origin(strings.clone()),
        ScriptAsset {
            label: &hotkeys,
            bytes: &keys,
        },
        origin(hotkeys.clone()),
        rules,
        Budget {
            bytes: 2 << 20,
            entries: 20000,
            string_bytes: 64 << 10,
        },
    )
    .unwrap();
    let missing = bank
        .localized_hotkey("SYNTHETIC_MISSING_LOCALIZATION_GATE_KEY")
        .unwrap();
    for key in [
        "GAMEOVER_RESTART",
        "GAMEOVER_REDUCE_DIFFICULTY",
        "GAMEOVER_LOAD",
        "GAMEOVER_QUIT_MISSION",
    ] {
        let text = bank.localized_string(key).unwrap().expect("selected label");
        assert_ne!(
            text, key,
            "selected key fell through to source missing-key fallback"
        );
        assert_ne!(
            bank.localized_hotkey(key).unwrap(),
            missing,
            "selected hotkey fell through to source default"
        );
    }
    assert_eq!(bank.strings.origin.source, strings);
    assert_eq!(bank.hotkeys.origin.source, hotkeys);
}
