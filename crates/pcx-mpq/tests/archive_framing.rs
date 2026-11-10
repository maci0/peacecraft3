//! Archive framing through the public readers: where the declared size may fall
//! short of the block table end, and which table, member and trailer layouts are
//! refused. Every archive is built here; none is an original asset.
use pcx_mpq::checked::{CheckedArchive, ReadBudget};
use pcx_mpq::extraction::ExtractionBudget;
use pcx_mpq::{Error, HeaderParseError, MpqArchive, crypt, flags, hash};

const MEMBER: &str = "framing.bin";
const PAYLOAD: &[u8] = b"stored-payload16";
/// Just past the 0x28-byte header.
const PAYLOAD_AT: u32 = 0x28;
const HASH_AT: u32 = 0x40;
const HASH_SLOTS: u32 = 32;
const BLOCK_AT: u32 = HASH_AT + HASH_SLOTS * 16;
/// The byte after the one block entry: the archive end of the synthetic layout.
const TABLES_END: u32 = BLOCK_AT + 16;
const BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 1 << 16,
    max_archive_table_bytes: 1 << 12,
    max_raw_packed_bytes: 1 << 12,
    max_decoded_bytes: 1 << 12,
};

/// The reader decrypts the tables, so the fixture encrypts them the way the
/// writer did.
fn encrypt(data: &mut [u8], key: u32) {
    let table = crypt::table();
    let mut seed1 = key;
    let mut seed2 = 0xEEEE_EEEEu32;
    for chunk in data.as_chunks_mut::<4>().0 {
        seed2 = seed2.wrapping_add(table[0x400 + (seed1 & 0xFF) as usize]);
        let plain = u32::from_le_bytes(*chunk);
        chunk.copy_from_slice(&(plain ^ seed1.wrapping_add(seed2)).to_le_bytes());
        seed1 = ((!seed1) << 0x15).wrapping_add(0x1111_1111) | (seed1 >> 0x0B);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
}

/// The 0x28-byte MPQ v1 header with the given fields.
fn header(
    declared: u32,
    hash_at: u32,
    block_at: u32,
    hash_entries: u32,
    block_entries: u32,
) -> Vec<u8> {
    let mut raw = b"MPQ\x1a".to_vec();
    raw.extend(0x20u32.to_le_bytes());
    raw.extend(declared.to_le_bytes());
    raw.extend(0u16.to_le_bytes());
    raw.push(3); // sector size shift: 4096-byte sectors
    raw.push(0);
    raw.extend(hash_at.to_le_bytes());
    raw.extend(block_at.to_le_bytes());
    raw.extend(hash_entries.to_le_bytes());
    raw.extend(block_entries.to_le_bytes());
    raw.extend([0u8; 8]);
    raw
}

/// A synthetic archive in the Legacy layout: the header, a stored member
/// payload at `PAYLOAD_AT`, the hash table at `HASH_AT`, one block entry after
/// it at `BLOCK_AT`, and `trailing` bytes after the tables. `declared` is the
/// header's archive size and `block_at`/`block_len` are the member's block entry.
fn archive(declared: u32, block_at: u32, block_len: u32, trailing: &[u8]) -> Vec<u8> {
    let mut raw = header(declared, HASH_AT, BLOCK_AT, HASH_SLOTS, 1);
    raw.resize(PAYLOAD_AT as usize, 0);
    raw.extend_from_slice(PAYLOAD);
    raw.resize(HASH_AT as usize, 0);

    // Every slot empty, then the member's name at its probe slot.
    let (name1, name2) = hash::hash_string_both(MEMBER);
    let slot = hash::start_slot(MEMBER, HASH_SLOTS as usize);
    let mut hashes = vec![0xffu8; (HASH_SLOTS * 16) as usize];
    let entry = &mut hashes[slot * 16..slot * 16 + 16];
    entry[..4].copy_from_slice(&name1.to_le_bytes());
    entry[4..8].copy_from_slice(&name2.to_le_bytes());
    entry[8..12].fill(0);
    entry[12..].copy_from_slice(&0u32.to_le_bytes());
    encrypt(&mut hashes, hash::hash_table_key());
    raw.extend_from_slice(&hashes);

    let mut block = Vec::new();
    for word in [
        block_at,
        block_len,
        block_len,
        flags::EXISTING | flags::SINGLE_UNIT,
    ] {
        block.extend(word.to_le_bytes());
    }
    encrypt(&mut block, hash::block_table_key());
    raw.extend_from_slice(&block);

    raw.extend_from_slice(trailing);
    raw
}

fn payload_len() -> u32 {
    PAYLOAD.len() as u32
}

#[test]
fn a_declared_size_short_of_the_block_table_reads_every_member() {
    // The shape of 25 loose 1.28c maps: the declared size is 3 to 255 bytes
    // short, the file ends at the block table end, and the member lies inside
    // the declared size.
    for short in [1u32, 3, 255] {
        let bytes = archive(TABLES_END - short, PAYLOAD_AT, payload_len(), &[]);
        let mut mpq = MpqArchive::from_bytes(bytes.clone(), "tail").expect("archive opens");
        assert_eq!(mpq.header.archive_size, TABLES_END - short);
        assert_eq!(mpq.byte_len(), u64::from(TABLES_END));
        assert_eq!(mpq.hash_entries().len(), HASH_SLOTS as usize);
        assert_eq!(mpq.block_entries().len(), 1);
        assert_eq!(
            mpq.extract(MEMBER).expect("member reads"),
            PAYLOAD,
            "short {short}"
        );

        let checked = CheckedArchive::parse(&bytes, "tail", BUDGET).expect("checked parse");
        assert_eq!(
            checked
                .extract(MEMBER, PAYLOAD.len())
                .expect("checked read"),
            Some(PAYLOAD.to_vec()),
            "checked short {short}"
        );
    }
}

#[test]
fn a_bounded_read_accepts_the_short_declared_size() {
    let bytes = archive(TABLES_END - 3, PAYLOAD_AT, payload_len(), &[]);
    let mut mpq = MpqArchive::from_bytes(bytes, "tail").expect("archive opens");
    let budget = ExtractionBudget {
        max_raw_packed_bytes: 1 << 12,
        max_decoded_bytes: 1 << 12,
    };
    let bounded = mpq
        .extract_bounded(MEMBER, budget, None)
        .expect("bounded read");
    assert_eq!(bounded, PAYLOAD);
}

#[test]
fn an_untagged_trailer_after_the_tables_is_outside_the_archive() {
    // Declared size equal to the table end: the trailing bytes are not part of
    // the archive, whatever their length, and the member still reads.
    for trailing_len in [1usize, 219] {
        let trailing = vec![0xA5u8; trailing_len];
        let bytes = archive(TABLES_END, PAYLOAD_AT, payload_len(), &trailing);
        let mut mpq = MpqArchive::from_bytes(bytes, "trailer").expect("archive opens");
        assert_eq!(mpq.header.archive_size, TABLES_END);
        assert_eq!(mpq.extract(MEMBER).expect("member reads"), PAYLOAD);
    }
}

#[test]
fn a_member_placed_in_the_trailer_is_refused() {
    // The member's block entry points into bytes past the archive end. Its
    // extent is inside the file, so only the archive bound refuses it.
    let bytes = archive(TABLES_END, TABLES_END, 4, &[0xA5; 8]);
    let mut mpq = MpqArchive::from_bytes(bytes, "trailer").expect("archive opens");
    assert!(matches!(
        mpq.extract(MEMBER),
        Err(Error::Unsupported("member lies outside the archive"))
    ));

    // The same member in a short-declared archive, whose file ends at the table end.
    let bytes = archive(TABLES_END - 3, TABLES_END, 4, &[]);
    let mut mpq = MpqArchive::from_bytes(bytes, "tail").expect("archive opens");
    assert!(matches!(
        mpq.extract(MEMBER),
        Err(Error::Unsupported("member lies outside the archive"))
    ));
}

#[test]
fn a_declared_size_past_the_file_is_refused() {
    let bytes = archive(TABLES_END + 1, PAYLOAD_AT, payload_len(), &[]);
    assert!(matches!(
        MpqArchive::from_bytes(bytes, "long"),
        Err(Error::Header(HeaderParseError::SizeMismatch { .. }))
    ));
}

#[test]
fn a_block_table_past_the_file_is_refused() {
    // The declared size fits the file, but the block table does not.
    let mut bytes = archive(TABLES_END - 3, PAYLOAD_AT, payload_len(), &[]);
    bytes.pop();
    assert!(matches!(
        MpqArchive::from_bytes(bytes, "truncated"),
        Err(Error::Header(HeaderParseError::TablesPastEnd {
            end,
            actual,
        })) if end == u64::from(TABLES_END) && actual == u64::from(TABLES_END - 1)
    ));
}

#[test]
fn a_declared_size_past_the_block_table_is_refused() {
    // Bytes after the tables that no table or member accounts for: the
    // declared size claims them, so the layout is inconsistent.
    let bytes = archive(TABLES_END + 4, PAYLOAD_AT, payload_len(), &[0; 16]);
    assert!(matches!(
        MpqArchive::from_bytes(bytes, "gap"),
        Err(Error::Unsupported("header table layout is inconsistent"))
    ));
}

#[test]
fn non_adjacent_tables_are_refused() {
    // The block table starts 16 bytes after the hash table's end. The file and
    // the declared size both reach the block table end, so only adjacency fails.
    let mut bytes = archive(TABLES_END + 16, PAYLOAD_AT, payload_len(), &[0; 16]);
    let moved = BLOCK_AT + 16;
    bytes[0x14..0x18].copy_from_slice(&moved.to_le_bytes());
    assert!(matches!(
        MpqArchive::from_bytes(bytes, "moved"),
        Err(Error::Unsupported("header table layout is inconsistent"))
    ));
}

#[test]
fn table_counts_past_the_file_are_refused_before_any_table_is_read() {
    // Maximal counts would need a 64 GiB table. The check runs on the header
    // alone, so the refusal is immediate and allocates nothing.
    let mut bytes = header(0x40, 0x40, 0x40, u32::MAX, u32::MAX);
    bytes.resize(0x40, 0);
    assert!(matches!(
        MpqArchive::from_bytes(bytes, "counts"),
        Err(Error::Header(HeaderParseError::TablesPastEnd { .. }))
    ));
}

#[test]
fn empty_and_truncated_archives_are_refused() {
    assert!(MpqArchive::from_bytes(Vec::new(), "empty").is_err());
    assert!(MpqArchive::from_bytes(b"MPQ\x1a".to_vec(), "magic only").is_err());
    let mut partial = archive(TABLES_END, PAYLOAD_AT, payload_len(), &[]);
    partial.truncate(0x1C);
    assert!(MpqArchive::from_bytes(partial, "partial header").is_err());
}
