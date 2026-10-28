//! Local-input check of snap compatibility with the two recorded Necropolis positions.
//! This does not run the placement search or establish the native texture-axis mapping.
use pcx_assets::ContentSource;
use pcx_objects::editor::{
    SlkId,
    defaults::{DefaultTables, Family},
};
use pcx_play::{
    ai_placement::Rules, campaign_client, campaign_inputs::client_identity,
    install_source::InstallSource,
};
use std::path::PathBuf;

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR and PCX_HUMAN05_MAP"]
fn recorded_necropolis_sites_are_snap_stable_with_stock_texture() {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect("set PEACECRAFT3_LEGACY_DIR"),
    );
    let map_path = PathBuf::from(std::env::var_os("PCX_HUMAN05_MAP").expect("set PCX_HUMAN05_MAP"));
    let image_path = install.join("Warcraft III.exe");
    let image = std::fs::read(&image_path).expect("selected executable");
    let identity = client_identity(&image, &image_path.display().to_string());
    let rules =
        Rules::reviewed_legacy(&image, identity.clone()).expect("selected AI placement rules");
    let cell_size = campaign_client::placement_rules(&image, identity)
        .expect("placement cell size")
        .cell_size();
    let map_bytes = std::fs::read(&map_path).expect("selected Human05 map");
    let map = pcx_map::Map::parse(&map_bytes).expect("map container");
    let budget = pcx_mpq::checked::ReadBudget {
        max_archive_bytes: 64 << 20,
        max_archive_table_bytes: 8 << 20,
        max_raw_packed_bytes: 16 << 20,
        max_decoded_bytes: 16 << 20,
    };
    let read = |name| {
        map.extract_checked(&map_path.display().to_string(), name, budget)
            .expect("map member")
            .expect("required map member")
    };
    let terrain = pcx_map::Terrain::from_bytes(read("war3map.w3e")).expect("terrain");
    let pathing = pcx_map::Pathing::parse(&read("war3map.wpm")).expect("pathing");
    pcx_map::grid_alignment::terrain_pathing_alignment(
        terrain.width(),
        terrain.height(),
        pathing.width,
        pathing.height,
    )
    .expect("map grid alignment");
    let (x, y) = terrain.origin();
    let bounds = [
        [x, y],
        [
            x + pathing.width as f32 * cell_size,
            y + pathing.height as f32 * cell_size,
        ],
    ];
    let tables = DefaultTables::open(&install, "1.29.2.9231").expect("selected unit tables");
    let unit = tables
        .object(Family::Unit, "unpl")
        .expect("stock Necropolis");
    let path = &unit
        .cell(SlkId::UnitData, "pathTex")
        .expect("stock pathing texture")
        .text;
    let source = InstallSource::open(&install).expect("selected install");
    let asset = source
        .read_bounded(path, 1 << 20)
        .expect("texture read")
        .expect("texture exists");
    let header = pcx_tga::Header::parse(&asset.bytes).expect("pathing texture header");
    let dimensions = [u32::from(header.width), u32::from(header.height)];
    for site in [[6208., 2752.], [5952., 2432.]] {
        assert_eq!(rules.snap(site, bounds, dimensions, 0).expect("snap"), site);
    }
}
