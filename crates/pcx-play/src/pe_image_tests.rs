use super::*;

pub(crate) const IMAGE_BASE: u32 = 0x400000;
pub(crate) const DATA: &SectionName = b".data\0\0\0";
const OPT_LEN: usize = 0xe0;

/// Minimal PE32 image: DOS stub, headers, then each section's raw bytes at `raw_at`.
pub(crate) fn synthetic_image(sections: &[(&SectionName, u32, &[u8])]) -> Vec<u8> {
    let headers_len = 0x400;
    let mut out = vec![0u8; headers_len];
    out[PE_OFFSET_AT..PE_OFFSET_AT + 4].copy_from_slice(&0x80u32.to_le_bytes());
    let pe = 0x80;
    out[pe..pe + 4].copy_from_slice(b"PE\0\0");
    let coff = pe + 4;
    out[coff + 2..coff + 4].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    out[coff + 16..coff + 18].copy_from_slice(&(OPT_LEN as u16).to_le_bytes());
    let opt = coff + COFF_LEN;
    out[opt..opt + 2].copy_from_slice(&PE32_MAGIC.to_le_bytes());
    out[opt + 28..opt + 32].copy_from_slice(&IMAGE_BASE.to_le_bytes());
    let table = opt + OPT_LEN;
    let mut raw_at = headers_len;
    for (i, (name, rva, bytes)) in sections.iter().enumerate() {
        let at = table + SECTION_LEN * i;
        out[at..at + 8].copy_from_slice(&name[..]);
        out[at + 8..at + 12].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        out[at + 12..at + 16].copy_from_slice(&rva.to_le_bytes());
        out[at + 16..at + 20].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        out[at + 20..at + 24].copy_from_slice(&(raw_at as u32).to_le_bytes());
        raw_at += bytes.len();
    }
    for (_, _, bytes) in sections {
        out.extend_from_slice(bytes);
    }
    out
}

#[test]
fn sections_resolve_by_name_and_virtual_addresses_stay_inside_raw_data() {
    let text = [0x55u8, 0x8b, 0xec, 0xc3];
    let data = [1u8, 0, 0, 0, 2, 0, 0, 0];
    let bytes = synthetic_image(&[(TEXT, 0x1000, &text), (DATA, 0x3000, &data)]);
    let pe = PeSections::parse(&bytes).unwrap();
    assert_eq!(pe.image_base, IMAGE_BASE);
    assert_eq!(pe.section(TEXT).unwrap().unwrap().bytes, &text);
    assert!(pe.section(RDATA).unwrap().is_none());
    assert_eq!(pe.va_bytes(IMAGE_BASE + 0x1000, 4), Some(&text[..]));
    assert_eq!(
        pe.va_bytes(IMAGE_BASE + 0x3004, 4),
        Some(&[2u8, 0, 0, 0][..])
    );
    // Past the raw bytes, before the next section, and below the image base: absent.
    assert_eq!(pe.va_bytes(IMAGE_BASE + 0x1000, 5), None);
    assert_eq!(pe.va_bytes(IMAGE_BASE + 0x2000, 1), None);
    assert_eq!(pe.va_bytes(IMAGE_BASE - 1, 1), None);
}

#[test]
fn malformed_headers_are_refused_with_a_reason() {
    assert_eq!(
        PeSections::parse(&[0u8; 8]).err(),
        Some("no PE header offset")
    );
    let mut bad = synthetic_image(&[(TEXT, 0x1000, &[0xc3])]);
    bad[0x80] = b'X';
    assert_eq!(PeSections::parse(&bad).err(), Some("missing PE signature"));
    let mut truncated = synthetic_image(&[(TEXT, 0x1000, &[0xc3])]);
    truncated.truncate(0x100);
    assert!(PeSections::parse(&truncated).is_err());
}

#[test]
fn a_repeated_name_is_refused_when_queried_and_unrelated_repeats_stay_accepted() {
    let bytes = synthetic_image(&[
        (TEXT, 0x1000, &[0xc3]),
        (TEXT, 0x2000, &[0xc3]),
        (DATA, 0x3000, &[0]),
        (DATA, 0x4000, &[0]),
    ]);
    let pe = PeSections::parse(&bytes).unwrap();
    assert_eq!(pe.section(TEXT).err(), Some("duplicate section"));
    assert!(pe.section(DATA).is_err());
    let single = synthetic_image(&[
        (TEXT, 0x1000, &[0xc3]),
        (DATA, 0x3000, &[0]),
        (DATA, 0x4000, &[0]),
    ]);
    assert!(
        PeSections::parse(&single)
            .unwrap()
            .section(RDATA)
            .unwrap()
            .is_none()
    );
}

#[test]
fn occurrences_equal_the_window_scan_at_every_boundary() {
    let mut state = 99u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u8 % 4
    };
    for len in [0usize, 1, 2, 31, 32, 33, 63, 64, 65, 200] {
        let haystack: Vec<u8> = (0..len).map(|_| next()).collect();
        for needle_len in 1..=4 {
            for seed in 0..8u8 {
                let needle: Vec<u8> = (0..needle_len).map(|n| (seed >> n) & 3).collect();
                let expected: Vec<usize> = haystack
                    .windows(needle_len)
                    .enumerate()
                    .filter(|(_, window)| *window == needle)
                    .map(|(at, _)| at)
                    .collect();
                let actual: Vec<usize> = occurrences(&haystack, &needle).collect();
                assert_eq!(actual, expected, "len {len} needle {needle:?}");
            }
        }
    }
    assert_eq!(occurrences(&[1, 2, 3], &[]).count(), 0);
    assert_eq!(occurrences(&[1, 2], &[1, 2, 3]).count(), 0);
    assert_eq!(occurrences(&[7; 70], &[7, 7]).count(), 69);
}

#[test]
fn windows_with_byte_equal_the_filtered_window_scan() {
    let mut state = 5u64;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u8 % 3
    };
    for len in [0usize, 1, 5, 31, 32, 33, 64, 97] {
        let haystack: Vec<u8> = (0..len).map(|_| next()).collect();
        for window_len in 1..=6 {
            for index in 0..window_len {
                for byte in 0..3u8 {
                    let expected: Vec<(usize, &[u8])> = haystack
                        .windows(window_len)
                        .enumerate()
                        .filter(|(_, window)| window[index] == byte)
                        .collect();
                    let actual: Vec<(usize, &[u8])> =
                        windows_with_byte(&haystack, index, byte, window_len).collect();
                    assert_eq!(actual, expected, "len {len} window {window_len} at {index}");
                }
            }
        }
    }
}
