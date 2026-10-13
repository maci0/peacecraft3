//! BLTE framing and unencrypted N/Z chunks. Unknown tags fail with a next probe.
use crate::{Error, Evidence, Failure, Key, Provenance, bytes, error, integer, verify_md5};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub encoded_bytes: u64,
    pub decoded_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Plain,
    Zlib,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub offset: usize,
    pub encoded_size: usize,
    pub decoded_size: usize,
    pub checksum: Option<Key>,
    pub mode: Mode,
}

#[derive(Debug)]
pub struct Decoded {
    pub evidence: Evidence,
    pub ekey: Key,
    pub content_key: Key,
    pub header_size: usize,
    pub chunks: Vec<Chunk>,
    pub bytes: Vec<u8>,
}

pub fn decode(
    data: &[u8],
    expected_ekey: Key,
    source: Provenance,
    limits: Limits,
) -> Result<Decoded, Error> {
    let name = &source.member;
    enforce_limit(data.len() as u64, limits.encoded_bytes, name, 0)?;
    if bytes(data, 0, 4, name)? != b"BLTE" {
        return Err(error(
            name,
            0,
            Failure::Unsupported {
                feature: "encoded signature",
                evidence: crate::hex(bytes(data, 0, 4, name)?),
                next_probe: "identify the actual envelope; BLDR is not a verified BLTE alias",
            },
        ));
    }
    let header_size = integer(bytes(data, 4, 4, name)?, true) as usize;
    let mut chunks = Vec::new();
    let mut output = Vec::new();
    if header_size == 0 {
        verify_md5(data, expected_ekey, name, 0)?;
        let raw = bytes(data, 8, data.len().saturating_sub(8), name)?;
        let (decoded, mode) = decode_chunk(raw, name, 8, limits.decoded_bytes)?;
        chunks.push(Chunk {
            offset: 8,
            encoded_size: raw.len(),
            decoded_size: decoded.len(),
            checksum: None,
            mode,
        });
        output = decoded;
    } else {
        let header = bytes(data, 0, header_size, name)?;
        verify_md5(header, expected_ekey, name, 0)?;
        if bytes(header, 8, 1, name)?[0] != 0x0f {
            return Err(error(
                name,
                8,
                Failure::Unsupported {
                    feature: "BLTE chunk-table flags",
                    evidence: format!("{:#x}", header[8]),
                    next_probe: "establish flagged chunk-descriptor framing",
                },
            ));
        }
        let count = integer(bytes(header, 9, 3, name)?, true) as usize;
        if count == 0
            || count.checked_mul(24).and_then(|size| size.checked_add(12)) != Some(header_size)
        {
            return Err(error(
                name,
                9,
                Failure::Malformed("chunk count does not consume the header exactly".into()),
            ));
        }
        let mut offset = header_size;
        for i in 0..count {
            let descriptor = bytes(header, 12 + i * 24, 24, name)?;
            let encoded_size = integer(&descriptor[..4], true) as usize;
            let decoded_size = integer(&descriptor[4..8], true) as usize;
            let checksum: Key = descriptor[8..].try_into().unwrap();
            enforce_limit(
                output.len() as u64 + decoded_size as u64,
                limits.decoded_bytes,
                name,
                offset,
            )?;
            let raw = bytes(data, offset, encoded_size, name)?;
            verify_md5(raw, checksum, name, offset)?;
            let (decoded, mode) = decode_chunk(raw, name, offset, decoded_size as u64)?;
            if decoded.len() != decoded_size {
                return Err(error(
                    name,
                    offset,
                    Failure::Malformed(format!(
                        "chunk decoded {} bytes, descriptor declares {decoded_size}",
                        decoded.len()
                    )),
                ));
            }
            output.extend_from_slice(&decoded);
            chunks.push(Chunk {
                offset,
                encoded_size,
                decoded_size,
                checksum: Some(checksum),
                mode,
            });
            offset = offset.checked_add(encoded_size).ok_or_else(|| {
                error(
                    name,
                    offset,
                    Failure::Malformed("chunk offset overflow".into()),
                )
            })?;
        }
        if offset != data.len() {
            return Err(error(
                name,
                offset,
                Failure::Malformed("trailing bytes beyond declared chunks".into()),
            ));
        }
    }
    let content_key = md5::compute(&output).0;
    Ok(Decoded {
        evidence: Evidence::of(source, data),
        ekey: expected_ekey,
        content_key,
        header_size,
        chunks,
        bytes: output,
    })
}

fn decode_chunk(
    raw: &[u8],
    source: &str,
    offset: usize,
    limit: u64,
) -> Result<(Vec<u8>, Mode), Error> {
    let (&tag, data) = raw
        .split_first()
        .ok_or_else(|| error(source, offset, Failure::Truncated { need: 1, have: 0 }))?;
    match tag {
        b'N' => {
            enforce_limit(data.len() as u64, limit, source, offset)?;
            Ok((data.to_vec(), Mode::Plain))
        }
        b'Z' => {
            let mut decoder = flate2::Decompress::new(true);
            let mut output = Vec::new();
            loop {
                let before_in = decoder.total_in();
                let before_out = decoder.total_out();
                let mut buffer = [0; 8192];
                let status = decoder
                    .decompress(
                        &data[before_in as usize..],
                        &mut buffer,
                        flate2::FlushDecompress::None,
                    )
                    .map_err(|err| {
                        error(source, offset, Failure::Malformed(format!("zlib: {err}")))
                    })?;
                let produced = (decoder.total_out() - before_out) as usize;
                enforce_limit(decoder.total_out(), limit, source, offset)?;
                output.extend_from_slice(&buffer[..produced]);
                if status == flate2::Status::StreamEnd {
                    break;
                }
                if decoder.total_in() == before_in && decoder.total_out() == before_out {
                    return Err(error(
                        source,
                        offset,
                        Failure::Malformed("zlib stream did not reach its end".into()),
                    ));
                }
            }
            if decoder.total_in() != data.len() as u64 {
                return Err(error(
                    source,
                    offset,
                    Failure::Malformed("zlib stream has trailing bytes".into()),
                ));
            }
            Ok((output, Mode::Zlib))
        }
        _ => Err(error(
            source,
            offset,
            Failure::Unsupported {
                feature: "BLTE chunk mode",
                evidence: format!("tag {tag:#04x}"),
                next_probe: "inspect this exact chunk's codec/key metadata; encryption and recursive frames are not decoded",
            },
        )),
    }
}

pub(crate) fn enforce_limit(
    needed: u64,
    limit: u64,
    source: &str,
    offset: usize,
) -> Result<(), Error> {
    if needed > limit {
        return Err(error(source, offset, Failure::Limit { needed, limit }));
    }
    Ok(())
}
