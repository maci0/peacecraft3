//! How field values are checked as UTF-8 and which error wins in a malformed record.
use pcx_slq::{Error, Table};

fn fixture(body: &[u8]) -> Vec<u8> {
    let mut input = b"ID;PWXL;N;E\r\n".to_vec();
    input.extend_from_slice(body);
    input.extend_from_slice(b"E\r\n");
    input
}

fn refused_as_not_utf8(body: &[u8]) -> bool {
    matches!(
        Table::parse(&fixture(body)),
        Err(Error::Unsupported {
            what: "field value is not UTF-8",
            ..
        })
    )
}

#[test]
fn multibyte_text_is_kept_whole() {
    let table = Table::parse(&fixture("C;X1;Y1;K\"h\u{e9}llo \u{20ac}\"\r\n".as_bytes())).unwrap();
    assert_eq!(table.get(1, 1), Some("h\u{e9}llo \u{20ac}"));
}

#[test]
fn a_value_without_a_tag_is_dropped_before_it_is_checked() {
    let table = Table::parse(&fixture(b"C;X1;Y1;K\"a\";\"\xff\"\r\n")).unwrap();
    assert_eq!(table.get(1, 1), Some("a"));
}

#[test]
fn a_character_split_across_two_fields_is_refused() {
    assert!(refused_as_not_utf8(b"C;X1;Y1;K\"\xe2\";Z\"\x82\xac\"\r\n"));
}

#[test]
fn a_field_tagged_with_a_non_ascii_byte_is_refused() {
    assert!(refused_as_not_utf8(b"C;X1;Y1;K\"a\";\xc3\xa9\"x\"\r\n"));
}

#[test]
fn a_bad_field_is_reported_before_a_later_unterminated_quote() {
    assert!(refused_as_not_utf8(b"C;X1;Y1;K\"\xff\";Z\"abc\r\n"));
}

#[test]
fn an_unterminated_quote_after_good_fields_is_reported_as_such() {
    assert!(matches!(
        Table::parse(&fixture(b"C;X1;Y1;K\"abc\r\n")),
        Err(Error::UnterminatedQuote { .. })
    ));
}

#[test]
fn the_unterminated_final_field_is_not_checked_as_text() {
    assert!(matches!(
        Table::parse(&fixture(b"C;X1;Y1;K\"a\";Z\"\xff\r\n")),
        Err(Error::UnterminatedQuote { .. })
    ));
}
