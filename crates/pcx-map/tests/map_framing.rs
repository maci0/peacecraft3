//! Map container framing through `Map::parse`: the archive end, the opaque
//! trailer, and the layouts refused. Every map is built here; none is an
//! original asset.
use pcx_map::{Error, MAP_ARCHIVE_OFFSET, Map};
use pcx_mpq::HeaderParseError;

/// The archive's block table ends here, past its 0x28-byte header and a
/// 32-slot hash table.
const TABLES_END: usize = 0x250;

/// A map file: `HM3W`, a name, zero padding to the archive, then an archive
/// header declaring `declared` bytes and `archive_len` bytes of archive.
/// `trailer` follows the archive.
fn map_file(declared: u32, archive_len: usize, trailer: &[u8]) -> Vec<u8> {
    let mut file = b"HM3W".to_vec();
    file.extend(0u32.to_le_bytes());
    file.extend(b"Framing\0");
    file.extend(1u32.to_le_bytes()); // field A
    file.extend(2u32.to_le_bytes()); // field B
    file.resize(MAP_ARCHIVE_OFFSET, 0);

    let mut archive = b"MPQ\x1a".to_vec();
    archive.extend(0x20u32.to_le_bytes()); // header size
    archive.extend(declared.to_le_bytes());
    archive.extend(0u16.to_le_bytes());
    archive.extend([3u8, 0]); // sector size shift, reserved
    archive.extend(0x40u32.to_le_bytes()); // hash table position
    archive.extend(0x240u32.to_le_bytes()); // block table position
    archive.extend(32u32.to_le_bytes()); // hash entries
    archive.extend(1u32.to_le_bytes()); // block entries
    archive.extend([0u8; 8]);
    archive.resize(archive_len, 0);

    file.extend(archive);
    file.extend(trailer);
    file
}

#[test]
fn a_declared_size_short_of_the_block_table_ends_the_archive_at_its_tables() {
    // The shape of 25 loose 1.28c maps: 3 to 255 bytes short, file ends at the
    // block table end.
    for short in [1usize, 3, 255] {
        let file = map_file((TABLES_END - short) as u32, TABLES_END, &[]);
        let map = Map::parse(&file).unwrap_or_else(|error| panic!("short {short}: {error}"));
        assert_eq!(map.header.archive_size, TABLES_END, "short {short}");
        assert_eq!(map.archive().len(), TABLES_END, "short {short}");
        assert!(map.header.trailer.is_none(), "short {short}");
        assert_eq!(&map.archive()[..4], b"MPQ\x1a");
    }
}

#[test]
fn an_ngis_trailer_is_kept_as_opaque_bytes() {
    let mut trailer = b"NGIS".to_vec();
    trailer.extend([0x5A; 256]);
    let file = map_file(TABLES_END as u32, TABLES_END, &trailer);
    let map = Map::parse(&file).expect("NGIS-tagged trailer parses");
    assert_eq!(map.header.archive_size, TABLES_END);
    assert_eq!(map.header.trailer.as_deref(), Some(trailer.as_slice()));
}

#[test]
fn untagged_trailers_of_any_length_are_kept_opaque() {
    // The two 1.28c maps with 1 and 219 untagged trailing bytes.
    for len in [1usize, 219] {
        let trailer = vec![0x33u8; len];
        let file = map_file(TABLES_END as u32, TABLES_END, &trailer);
        let map = Map::parse(&file).unwrap_or_else(|error| panic!("trailer {len}: {error}"));
        assert_eq!(map.header.trailer.as_deref(), Some(trailer.as_slice()));
        assert_eq!(map.archive().len(), TABLES_END);
    }
}

#[test]
fn a_declared_size_past_the_file_is_refused() {
    let file = map_file((TABLES_END + 1) as u32, TABLES_END, &[]);
    assert!(matches!(
        Map::parse(&file),
        Err(Error::Archive(HeaderParseError::SizeMismatch { .. }))
    ));
}

#[test]
fn a_block_table_past_the_file_is_refused() {
    let file = map_file((TABLES_END - 3) as u32, TABLES_END - 1, &[]);
    assert!(matches!(
        Map::parse(&file),
        Err(Error::Archive(HeaderParseError::TablesPastEnd { .. }))
    ));
}

#[test]
fn a_declared_size_past_the_block_table_is_refused_when_the_archive_opens() {
    // Bytes the declared size claims but no table accounts for. The framing
    // parse keeps them out of the archive; opening the archive refuses the layout.
    let file = map_file((TABLES_END + 4) as u32, TABLES_END + 16, &[]);
    let map = Map::parse(&file).expect("framing holds: the block table ends inside the file");
    assert_eq!(map.archive().len(), TABLES_END);
    // The archive slice ends at the table end, so the declared size no longer fits it.
    assert!(matches!(
        map.open_archive(),
        Err(pcx_mpq::Error::Header(
            HeaderParseError::SizeMismatch { .. }
        ))
    ));
}

#[test]
fn a_truncated_archive_header_is_refused() {
    let file = map_file(TABLES_END as u32, 0x10, &[]);
    assert!(matches!(
        Map::parse(&file),
        Err(Error::Archive(HeaderParseError::Io(_)))
    ));
}

#[test]
fn empty_input_is_refused() {
    assert!(Map::parse(&[]).is_err());
}
