//! Explicit local-input probe of the blight columns in `TerrainArt\Terrain.slk`.
//! Prints the authored rows only; nothing is written to disk.

use pcx_mpq::MpqSet;

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR pointing at the selected Legacy install"]
fn selected_install_blight_columns() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .ok_or("set PEACECRAFT3_LEGACY_DIR to the selected 1.29.2.9231 install")?;
    let mut set = MpqSet::open_wc3(&dir)?;
    let table = pcx_slq::Table::parse(&set.extract("TerrainArt\\Terrain.slk")?)?;
    println!(
        "rows={} columns={}",
        table.row_count(),
        table.column_count()
    );
    for column in ["tileID", "blightPri", "convertTo", "walkable", "name"] {
        assert!(table.column_of(column).is_some(), "missing {column}");
    }
    for row in table.data_rows() {
        println!(
            "tileID={:?} name={:?} blightPri={:?} convertTo={:?} walkable={:?}",
            row.by_name("tileID"),
            row.by_name("name"),
            row.by_name("blightPri"),
            row.by_name("convertTo"),
            row.by_name("walkable"),
        );
    }
    Ok(())
}
