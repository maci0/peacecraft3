//! EN v1 content-key records, with all page-directory hashes verified.
//! ESpec execution, EKey-page payload interpretation and the final tail remain
//! opaque; the verified CKey records suffice for content-to-encoding joins.
use std::collections::BTreeMap;

use crate::{Error, Evidence, Failure, Key, Provenance, bytes, error, integer, verify_md5};

#[derive(Debug, Clone)]
pub struct Content {
    pub decoded_size: u64,
    pub ekeys: Vec<Key>,
    pub page: usize,
    pub page_offset: usize,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub version: u8,
    pub ckey_width: u8,
    pub ekey_width: u8,
    pub ckey_page_size: usize,
    pub ekey_page_size: usize,
    pub ckey_page_count: usize,
    pub ekey_page_count: usize,
    pub opaque_flag: u8,
    pub spec_size: usize,
}

#[derive(Debug)]
pub struct Encoding {
    pub evidence: Evidence,
    pub header: Header,
    pub contents: BTreeMap<Key, Content>,
    pub ckey_hashes_verified: usize,
    pub ekey_hashes_verified: usize,
    pub opaque_specs: Vec<u8>,
    /// Hash-verified but not interpreted as ESpec instructions.
    pub opaque_ekey_pages: Vec<u8>,
    pub opaque_tail: Vec<u8>,
}

impl Encoding {
    pub fn parse(data: &[u8], source: Provenance) -> Result<Self, Error> {
        let name = &source.member;
        let raw = bytes(data, 0, 22, name)?;
        if &raw[..2] != b"EN" || raw[2] != 1 || raw[3] != 16 || raw[4] != 16 {
            return Err(error(
                name,
                0,
                Failure::Unsupported {
                    feature: "ENCODING header",
                    evidence: format!(
                        "signature {} version {} key widths {}/{}",
                        crate::hex(&raw[..2]),
                        raw[2],
                        raw[3],
                        raw[4]
                    ),
                    next_probe: "establish version-specific record widths before walking pages",
                },
            ));
        }
        let header = Header {
            version: raw[2],
            ckey_width: raw[3],
            ekey_width: raw[4],
            ckey_page_size: integer(&raw[5..7], true) as usize * 1024,
            ekey_page_size: integer(&raw[7..9], true) as usize * 1024,
            ckey_page_count: integer(&raw[9..13], true) as usize,
            ekey_page_count: integer(&raw[13..17], true) as usize,
            opaque_flag: raw[17],
            spec_size: integer(&raw[18..22], true) as usize,
        };
        if header.opaque_flag != 0 {
            return Err(error(
                name,
                17,
                Failure::Unsupported {
                    feature: "ENCODING flag",
                    evidence: header.opaque_flag.to_string(),
                    next_probe: "establish flag effects on manifest framing",
                },
            ));
        }
        if header.ckey_page_size == 0 || header.ekey_page_size == 0 {
            return Err(error(name, 5, Failure::Malformed("zero page size".into())));
        }
        let specs = bytes(data, 22, header.spec_size, name)?;
        let mut offset = 22 + specs.len();
        let ckey_directory_size = checked_product(
            header.ckey_page_count,
            usize::from(header.ckey_width) + 16,
            name,
            offset,
        )?;
        let ckey_directory = bytes(data, offset, ckey_directory_size, name)?;
        offset += ckey_directory.len();
        let ckey_data_size =
            checked_product(header.ckey_page_count, header.ckey_page_size, name, offset)?;
        let ckey_data = bytes(data, offset, ckey_data_size, name)?;
        let ckey_offset = offset;
        offset += ckey_data.len();
        let ekey_directory_size = checked_product(
            header.ekey_page_count,
            usize::from(header.ekey_width) + 16,
            name,
            offset,
        )?;
        let ekey_directory = bytes(data, offset, ekey_directory_size, name)?;
        offset += ekey_directory.len();
        let ekey_data_size =
            checked_product(header.ekey_page_count, header.ekey_page_size, name, offset)?;
        let ekey_data = bytes(data, offset, ekey_data_size, name)?;
        let ekey_offset = offset;
        offset += ekey_data.len();
        let mut contents = BTreeMap::new();
        for (i, page) in ckey_data.chunks_exact(header.ckey_page_size).enumerate() {
            let directory = &ckey_directory[i * 32..(i + 1) * 32];
            verify_md5(
                page,
                directory[16..].try_into().unwrap(),
                name,
                ckey_offset + i * header.ckey_page_size,
            )?;
            if bytes(page, 6, 16, name)? != &directory[..16] {
                return Err(error(
                    name,
                    ckey_offset + i * header.ckey_page_size,
                    Failure::Malformed("CKey page first key differs from directory".into()),
                ));
            }
            let mut cursor = 0;
            let mut previous: Option<Key> = None;
            while cursor < page.len() {
                let count = integer(bytes(page, cursor, 2, name)?, false) as usize;
                if count == 0 {
                    if page[cursor..].iter().any(|&byte| byte != 0) {
                        return Err(error(
                            name,
                            ckey_offset + i * header.ckey_page_size + cursor,
                            Failure::Unsupported {
                                feature: "CKey page padding",
                                evidence: "nonzero bytes after zero-count terminator".into(),
                                next_probe: "identify a different CKey record layout or terminator",
                            },
                        ));
                    }
                    break;
                }
                let size = integer(bytes(page, cursor + 2, 4, name)?, true);
                let ckey: Key = bytes(page, cursor + 6, 16, name)?.try_into().unwrap();
                let ekey_bytes =
                    checked_product(count, usize::from(header.ekey_width), name, cursor)?;
                let raw_keys = bytes(page, cursor + 22, ekey_bytes, name)?;
                let ekeys = raw_keys.as_chunks::<16>().0.to_vec();
                if previous.is_some_and(|key| key >= ckey) || contents.contains_key(&ckey) {
                    return Err(error(
                        name,
                        ckey_offset + i * header.ckey_page_size + cursor,
                        Failure::Malformed("duplicate or unordered CKey".into()),
                    ));
                }
                previous = Some(ckey);
                contents.insert(
                    ckey,
                    Content {
                        decoded_size: size,
                        ekeys,
                        page: i,
                        page_offset: cursor,
                    },
                );
                cursor += 22 + ekey_bytes;
            }
        }
        for (i, page) in ekey_data.chunks_exact(header.ekey_page_size).enumerate() {
            let directory = &ekey_directory[i * 32..(i + 1) * 32];
            verify_md5(
                page,
                directory[16..].try_into().unwrap(),
                name,
                ekey_offset + i * header.ekey_page_size,
            )?;
            if page[..16] != directory[..16] {
                return Err(error(
                    name,
                    ekey_offset + i * header.ekey_page_size,
                    Failure::Malformed("EKey page first key differs from directory".into()),
                ));
            }
        }
        Ok(Self {
            evidence: Evidence::of(source, data),
            ckey_hashes_verified: header.ckey_page_count,
            ekey_hashes_verified: header.ekey_page_count,
            header,
            contents,
            opaque_specs: specs.to_vec(),
            opaque_ekey_pages: ekey_data.to_vec(),
            opaque_tail: data[offset..].to_vec(),
        })
    }

    pub fn interpret_specs(&self) -> Error {
        error(
            &self.evidence.source.member,
            22,
            Failure::Unsupported {
                feature: "ENCODING ESpec and EKey-page interpretation",
                evidence: format!(
                    "{} spec bytes, {} hash-verified EKey pages, {} trailing bytes retained",
                    self.opaque_specs.len(),
                    self.ekey_hashes_verified,
                    self.opaque_tail.len()
                ),
                next_probe: "verify the spec grammar and final-page sentinel against the supplied manifest and a native reader",
            },
        )
    }
}

fn checked_product(
    count: usize,
    width: usize,
    source: &str,
    offset: usize,
) -> Result<usize, Error> {
    count.checked_mul(width).ok_or_else(|| {
        error(
            source,
            offset,
            Failure::Malformed("page/count size overflow".into()),
        )
    })
}
