//! Explicit local gate: the Holy Light `Targetart` model has one non-looping `SEQS` record,
//! which is the lifetime `pcx-play` derives. Run with `PEACECRAFT3_FORMAT_INSTALL` set to
//! the selected install directory.
use pcx_mdl::Mdlx;
use pcx_mpq::MpqSet;

const HOLY_LIGHT_ART: &str = r"Abilities\Spells\Human\HolyBolt\HolyBoltSpecialArt.mdx";

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL pointing to a local original install"]
fn holy_light_art_has_one_non_looping_sequence() {
    let install = std::env::var_os("PEACECRAFT3_FORMAT_INSTALL")
        .expect("set PEACECRAFT3_FORMAT_INSTALL to the original install");
    let mut archives = MpqSet::open_wc3(&install).expect("open original install");
    let data = archives
        .extract(HOLY_LIGHT_ART)
        .expect("holy light art model");
    let model = Mdlx::parse(&data).expect("parse model");
    let sequences = model.sequences(&data).expect("SEQS");
    let [only] = sequences.as_slice() else {
        panic!("expected one sequence, found {}", sequences.len());
    };
    assert_eq!(only.name, "Birth");
    assert_eq!((only.interval_start, only.interval_end), (0, 2800));
    assert!(only.non_looping, "the art must not loop");
}
