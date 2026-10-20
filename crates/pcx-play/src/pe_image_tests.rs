use super::*;

pub(crate) const IMAGE_BASE: u32 = 0x400000;
pub(crate) const DATA: &SectionName = b".data\0\0\0";
const OPT_LEN: usize = 0xe0;

/// Minimal PE32 image: DOS stub, headers, then each section's raw bytes at `raw_at`.
pub(crate) fn synthetic_image(sections: &[(&SectionName, u32, &[u8])]) -> Vec<u8> {
    let headers_len = 0x200;
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
    assert_eq!(pe.section(TEXT).unwrap().bytes, &text);
    assert!(pe.section(RDATA).is_none());
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
