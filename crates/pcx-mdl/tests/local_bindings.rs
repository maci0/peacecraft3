//! Explicit local original-fixture checks; models are imported only in memory.
use pcx_mdl::{Mdlx, NO_ID, tag};
use pcx_mpq::MpqSet;

#[test]
#[ignore = "requires PEACECRAFT3_FORMAT_INSTALL pointing to a local original install"]
fn original_geoset_bindings_and_helper_tracks() {
    let install = std::env::var_os("PEACECRAFT3_FORMAT_INSTALL")
        .expect("set PEACECRAFT3_FORMAT_INSTALL to the original install");
    let mut archives = MpqSet::open_wc3(&install).expect("open original install");
    let paths = archives.file_list().expect("read original listfiles");
    let mut models = 0;
    let mut bindings = 0;
    let mut helper_tracks = 0;
    for path in paths
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".mdx"))
    {
        let data = archives
            .extract(path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let model = Mdlx::parse(&data).unwrap_or_else(|error| panic!("{path}: {error}"));
        let geosets = model.geosets(&data).unwrap();
        for anim in model.geoset_anims(&data).unwrap() {
            assert!(
                anim.geoset_id == NO_ID || (anim.geoset_id as usize) < geosets.len(),
                "{path}: GEOA references missing geoset {}",
                anim.geoset_id
            );
            bindings += 1;
        }
        for helper in model.nodes(&data, tag::HELP).unwrap() {
            helper_tracks += helper.tracks.len();
        }
        for emitter in model.particles2(&data).unwrap() {
            assert_eq!(
                emitter.properties().to_bytes(),
                emitter.state,
                "{path}: PRE2 properties round trip"
            );
        }
        models += 1;
    }
    assert!(
        models > 0 && bindings > 0 && helper_tracks > 0,
        "fixture must exercise original models, GEOA and animated HELP"
    );
    eprintln!(
        "install={install:?} models={models} geoset_bindings={bindings} helper_tracks={helper_tracks}"
    );
}
