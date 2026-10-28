//! Cell storage behavior that must not depend on the order records arrive in.
use pcx_slq::{AnomalyKind, Error, Table};

fn fixture(body: &[u8]) -> Vec<u8> {
    let mut input = b"ID;PWXL;N;E\r\n".to_vec();
    input.extend_from_slice(body);
    input.extend_from_slice(b"E\r\n");
    input
}

#[test]
fn cells_arriving_out_of_order_are_all_kept_in_coordinate_order() {
    let input =
        fixture(b"C;X2;Y1;K\"b1\"\r\nC;X1;Y1;K\"a1\"\r\nC;X1;Y2;K\"a2\"\r\nC;X3;Y1;K\"c1\"\r\n");
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.cell_count(), 4);
    assert_eq!(
        table.canonical(),
        b"Y1;X1;Ka1\nY1;X2;Kb1\nY1;X3;Kc1\nY2;X1;Ka2\n"
    );
    assert!(
        table
            .anomalies()
            .iter()
            .all(|a| a.kind != AnomalyKind::DuplicateCell)
    );
}

#[test]
fn a_repeat_after_an_out_of_order_cell_keeps_the_first_value() {
    // Record indices count from the record after `I`: the cells are 1, 2 and 3.
    let input = fixture(b"C;X1;Y2;K\"first\"\r\nC;X1;Y1;K\"h\"\r\nC;X1;Y2;K\"second\"\r\n");
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.get(2, 1), Some("first"));
    let repeats: Vec<_> = table
        .anomalies()
        .iter()
        .filter(|a| a.kind == AnomalyKind::DuplicateCell)
        .map(|a| (a.row, a.column, a.record))
        .collect();
    assert_eq!(repeats, [(Some(2), Some(1), 3)]);
}

#[test]
fn a_repeat_inside_an_ascending_run_is_reported_with_its_record() {
    let input = fixture(b"C;X1;Y1;K\"h\"\r\nC;X1;Y1;K\"again\"\r\nC;X2;Y1;K\"i\"\r\n");
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.get(1, 1), Some("h"));
    assert_eq!(table.get(1, 2), Some("i"));
    let repeats: Vec<_> = table
        .anomalies()
        .iter()
        .filter(|a| a.kind == AnomalyKind::DuplicateCell)
        .map(|a| a.record)
        .collect();
    assert_eq!(repeats, [2]);
}

#[test]
fn a_repeated_header_name_resolves_to_its_lowest_column() {
    let input = fixture(
        b"C;X1;Y1;K\"Name\"\r\nC;X2;Y1;K\"Other\"\r\nC;X3;Y1;K\"Name\"\r\nC;X1;Y2;K\"low\"\r\nC;X3;Y2;K\"high\"\r\n",
    );
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.column_of("Name"), Some(1));
    assert_eq!(table.column_of("Other"), Some(2));
    assert_eq!(table.column_of("Missing"), None);
    assert_eq!(table.by_name(2, "Name"), Some("low"));
    assert_eq!(table.header(3), Some("Name"));
}

#[test]
fn a_lone_line_feed_or_carriage_return_inside_a_record_is_value_text() {
    let input = fixture(b"C;X1;Y1;K\"a\nb\"\r\nC;X2;Y1;K\"c\rd\"\r\n");
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.get(1, 1), Some("a\nb"));
    assert_eq!(table.get(1, 2), Some("c\rd"));
}

#[test]
fn a_field_that_is_not_utf8_is_refused() {
    let input = fixture(b"C;X1;Y1;K\"\xff\"\r\n");
    assert!(matches!(
        Table::parse(&input),
        Err(Error::Unsupported {
            what: "field value is not UTF-8",
            ..
        })
    ));
}

#[test]
fn values_split_across_quoted_and_bare_runs_are_joined() {
    let input = fixture(b"C;X1;Y1;Kab\"c;d\"\"e\"fg\r\n");
    let table = Table::parse(&input).unwrap();
    assert_eq!(table.get(1, 1), Some("abc;d\"efg"));
}
