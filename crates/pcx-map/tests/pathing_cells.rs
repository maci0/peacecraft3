use pcx_map::Pathing;

#[test]
fn public_dimensions_cannot_wrap_a_query_onto_an_existing_cell() {
    let mut pathing = Pathing {
        version: 0,
        width: 1,
        height: 1,
        cells: vec![77],
    };
    assert_eq!(pathing.cell(0, 0), Some(77));
    pathing.width = 65_536;
    pathing.height = 65_537;
    assert_eq!(pathing.cell(0, 65_536), None);
    pathing.width = u32::MAX;
    pathing.height = 2;
    assert_eq!(pathing.cell(1, 1), None);
}

#[test]
fn caller_mutation_keeps_missing_and_out_of_grid_cells_distinct_from_values() {
    let pathing = Pathing {
        version: 0,
        width: 3,
        height: 2,
        cells: vec![0, 17, 99, 128],
    };
    assert_eq!(pathing.cell(0, 0), Some(0));
    assert_eq!(pathing.cell(0, 1), Some(128));
    assert_eq!(pathing.cell(1, 1), None);
    assert_eq!(pathing.cell(3, 0), None);
    assert_eq!(pathing.cell(0, 2), None);
}
