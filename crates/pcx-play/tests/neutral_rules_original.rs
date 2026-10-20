//! Original-input gate: production neutral-player rules discovered from the selected
//! executable. Budgets match `peacecraft3::campaign_inputs::prepare_new_game`.
use pcx_play::map_startup_host::NeutralRuleSet;
use pcx_play::neutral_player::{DiscoveryBudget, RuleInputBudget};
use pcx_play::scripts::AssetIdentity;

#[test]
#[ignore = "requires PCX_WAR3_EXE naming the selected Legacy Warcraft III.exe"]
fn selected_image_admits_the_six_neutral_rules() {
    let path = std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
    let image = std::fs::read(path).unwrap();
    let rules = NeutralRuleSet::discover_first_family(
        &image,
        AssetIdentity {
            edition: "legacy".into(),
            version: "1.29.2.9231".into(),
            source: "local selected executable".into(),
        },
        DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
            registration_candidates: 100000,
            work_bytes: 256 << 20,
        },
    )
    .unwrap_or_else(|e| panic!("{e:?}"));
    // Six distinct rules, each bound to this image.
    let images: Vec<_> = [
        &rules.passive,
        &rules.aggressive,
        &rules.max_players,
        &rules.victim,
        &rules.extra,
        &rules.slots,
    ]
    .iter()
    .map(|rule| rule.image_sha256())
    .collect();
    assert!(images.iter().all(|digest| *digest == images[0]));
}
