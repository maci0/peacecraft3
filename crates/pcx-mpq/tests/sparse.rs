//! Synthetic sparse decompression parity tests.
#[path = "../src/sparse.rs"]
mod sparse;

use sparse::{decompress, SparseError};

fn header(size: u32) -> Vec<u8> {
    size.to_be_bytes().to_vec()
}

#[test]
fn decodes_zero_run_from_single_marker() {
    let mut input = header(3);
    input.push(0x00);
    let mut out = [0xff; 3];
    assert_eq!(decompress(&input, &mut out).unwrap(), 3);
    assert_eq!(out, [0, 0, 0]);
}

#[test]
fn decodes_nonzero_run() {
    let mut input = header(4);
    input.push(0x83); // 4 nonzeros
    input.extend_from_slice(b"ABCD");
    let mut out = [0; 4];
    assert_eq!(decompress(&input, &mut out).unwrap(), 4);
    assert_eq!(&out, b"ABCD");
}

#[test]
fn decodes_max_zero_run_marker() {
    let mut input = header(130);
    input.push(0x7f);
    let mut out = [0xaa; 130];
    assert_eq!(decompress(&input, &mut out).unwrap(), 130);
    assert!(out.iter().all(|&b| b == 0));
}

#[test]
fn decodes_mixed_runs() {
    let mut input = header(6);
    input.push(0x00); // 3 zeros
    input.push(0x81); // 2 nonzeros
    input.extend_from_slice(b"XY");
    input.push(0x80); // 1 nonzero
    input.push(b'Z');
    let mut out = [0xff; 6];
    assert_eq!(decompress(&input, &mut out).unwrap(), 6);
    assert_eq!(&out, b"\0\0\0XYZ".as_slice());
}

#[test]
fn rejects_declared_size_larger_than_output_capacity() {
    let mut input = header(8);
    input.push(0x87);
    input.extend_from_slice(b"12345678");
    let mut out = [0; 4];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::DecodedSizeExceeds {
            decoded: 8,
            capacity: 4
        })
    ));
}

#[test]
fn rejects_truncated_nonzero_run() {
    let mut input = header(4);
    input.push(0x83); // needs 4
    input.extend_from_slice(b"ABC"); // only 3
    let mut out = [0; 4];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::Truncated { .. })
    ));
}

#[test]
fn rejects_partial_termination() {
    let mut input = header(4);
    input.push(0x82); // 3
    input.extend_from_slice(b"ABC");
    let mut out = [0; 4];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::SizeMismatch {
            decoded: 4,
            produced: 3
        })
    ));
}

#[test]
fn rejects_empty_declared_size_with_trailing_marker() {
    let mut input = header(0);
    input.push(0x00);
    let mut out = [];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::SizeMismatch {
            decoded: 0,
            produced: 0
        })
    ));
}

#[test]
fn accepts_empty_stream() {
    let input = header(0);
    let mut out = [];
    assert_eq!(decompress(&input, &mut out).unwrap(), 0);
}

#[test]
fn rejects_extra_input_after_complete_output() {
    let mut input = header(3);
    input.push(0x00); // 3 zeros consumes output
    input.push(0x80); // extra marker
    input.push(b'X');
    let mut out = [0; 3];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::SizeMismatch { .. })
    ));
}

#[test]
fn rejects_input_shorter_than_header() {
    let input = [0u8, 0, 0];
    let mut out = [];
    assert!(matches!(
        decompress(&input, &mut out),
        Err(SparseError::Truncated { need: 4, have: 3 })
    ));
}
