use pcx_slq::{AnomalyKind, CoordinateLimits, Error, Severity, Table};

fn fixture(body: &str) -> Vec<u8> {
    format!("ID;PWXL;N;E\r\n{body}E\r\n").into_bytes()
}
fn limits(rows: u32, columns: u32) -> CoordinateLimits {
    CoordinateLimits {
        max_row: rows,
        max_column: columns,
    }
}
fn duplicates(table: &Table) -> Vec<(u32, u32, usize)> {
    table
        .anomalies()
        .iter()
        .filter(|a| a.kind == AnomalyKind::DuplicateCell)
        .map(|a| {
            assert_eq!(a.severity, Severity::Warning);
            (a.row.unwrap(), a.column.unwrap(), a.record)
        })
        .collect()
}

#[test]
fn sparse_coordinates_refuse_before_grid_postprocessing() {
    for (body, expected_axis, expected_actual) in [
        ("C;X4294967295;Y1;K\"x\"\r\n", 'X', u32::MAX),
        ("C;X1;Y4294967295;K\"x\"\r\n", 'Y', u32::MAX),
        ("C;X4294967295;Y1\r\n", 'X', u32::MAX),
        ("F;X4294967295;Y1\r\n", 'X', u32::MAX),
    ] {
        let input = fixture(body);
        let error = Table::parse_bounded(&input, limits(3, 4)).unwrap_err();
        assert!(
            matches!(error, Error::CoordinateLimit { index: 1, offset: 13, axis, actual, limit }
            if axis == expected_axis && actual == expected_actual && limit == if axis == 'X' {4} else {3})
        );
    }
}

#[test]
fn exact_coordinate_boundaries_and_unbounded_entry() {
    let input = fixture("B;X4294967295;Y4294967295\r\nC;X4;Y3;K\"boundary\"\r\n");
    let bounded = Table::parse_bounded(&input, limits(3, 4)).unwrap();
    let legacy = Table::parse(&input).unwrap();
    assert_eq!(bounded.get(3, 4), Some("boundary"));
    assert_eq!(bounded.canonical(), legacy.canonical());
    assert_eq!(bounded.declared().unwrap().rows, u32::MAX);
    for policy in [limits(2, 4), limits(3, 3), limits(0, 4), limits(3, 0)] {
        assert!(matches!(
            Table::parse_bounded(&input, policy),
            Err(Error::CoordinateLimit { .. })
        ));
    }
    let input = fixture("C;X5;Y1;K\"outside caller policy\"\r\n");
    assert_eq!(
        Table::parse(&input).unwrap().get(1, 5),
        Some("outside caller policy")
    );
    assert!(Table::parse_bounded(&input, limits(3, 4)).is_err());
}

#[test]
fn format_cursor_is_checked_and_shared() {
    let input = fixture("F;X2;Y3\r\nC;K\"first\"\r\nF;X4\r\nC;K\"second\"\r\n");
    let table = Table::parse_bounded(&input, limits(3, 4)).unwrap();
    assert_eq!(table.get(3, 2), Some("first"));
    assert_eq!(table.get(3, 4), Some("second"));
    for (body, axis, index) in [
        ("F;X2;Y4\r\nC;K\"unreached\"\r\n", 'Y', 1),
        ("C;X1;Y1;K\"safe\"\r\nF;X5\r\nF;X1\r\n", 'X', 2),
    ] {
        assert!(matches!(Table::parse_bounded(&fixture(body), limits(3, 4)),
            Err(Error::CoordinateLimit { axis: actual, index: record, .. }) if actual == axis && record == index));
    }
    assert!(matches!(
        Table::parse_bounded(&fixture("F;X0;Y1\r\n"), limits(3, 4)),
        Err(Error::ZeroIndex { .. })
    ));
    assert!(matches!(
        Table::parse_bounded(&fixture("C;K\"implicit cursor\"\r\n"), limits(0, 4)),
        Err(Error::CoordinateLimit {
            axis: 'Y',
            actual: 1,
            ..
        })
    ));
}

#[test]
fn repeated_equal_and_different_writes_keep_first_value() {
    let input = fixture("C;X2;Y2;K\"first\"\r\nC;K\"first\"\r\nF;X2;Y2\r\nC;K\"different\"\r\n");
    for table in [
        Table::parse(&input).unwrap(),
        Table::parse_bounded(&input, limits(2, 2)).unwrap(),
    ] {
        assert_eq!(table.get(2, 2), Some("first"));
        assert_eq!(table.cell_count(), 1);
        assert_eq!(duplicates(&table), [(2, 2, 2), (2, 2, 4)]);
    }
}

#[test]
fn header_duplicate_and_quoted_values_keep_original_text() {
    let input = fixture(
        "C;X1;Y1;K\"tileID\"\r\nC;K\"otherID\"\r\nC;X1;Y2;K\"a;\"\"b\"\r\nC;K\"different\"\r\n",
    );
    let table = Table::parse_bounded(&input, limits(2, 1)).unwrap();
    assert_eq!(table.header(1), Some("tileID"));
    assert_eq!(table.get(1, 1), Some("tileID"));
    assert_eq!(table.get(2, 1).unwrap().as_bytes(), b"a;\"b");
    assert_eq!(duplicates(&table), [(1, 1, 2), (2, 1, 4)]);
}

#[test]
fn missing_k_then_valid_write_is_not_a_duplicate() {
    for (first, kind, severity) in [
        (
            "C;X1;Y2\r\n",
            AnomalyKind::CellValueMissing,
            Severity::Error,
        ),
        (
            "C;X1;Y2;A\"comment\"\r\n",
            AnomalyKind::CellValueUnknown,
            Severity::Warning,
        ),
    ] {
        let input = fixture(&format!("{first}C;K\"later valid\"\r\n"));
        let table = Table::parse_bounded(&input, limits(2, 1)).unwrap();
        assert_eq!(table.get(2, 1), Some("later valid"));
        assert!(duplicates(&table).is_empty());
        assert!(
            table
                .anomalies()
                .iter()
                .any(|a| a.kind == kind && a.severity == severity)
        );
    }
}

#[test]
fn repeated_k_fields_and_generated_state_preserve_existing_values() {
    let input = fixture(
        "C;X1;Y1;K\"first\";K\"first\";K\"different\";E\"formula\"\r\nC;K\"later\";E\"other\"\r\n",
    );
    let table = Table::parse_bounded(&input, limits(1, 1)).unwrap();
    assert_eq!(table.get(1, 1), Some("first"));
    assert_eq!(duplicates(&table), [(1, 1, 1), (1, 1, 1), (1, 1, 2)]);
    assert_eq!(
        table.generated_cells().iter().copied().collect::<Vec<_>>(),
        [(1, 1)]
    );
    assert_eq!(
        table
            .anomalies()
            .iter()
            .filter(|a| a.kind == AnomalyKind::GeneratedCell)
            .count(),
        2
    );
}
