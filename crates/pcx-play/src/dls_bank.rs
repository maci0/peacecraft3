//! Bounded DLS (Downloadable Sounds) bank parsing: a RIFF `DLS ` form with instruments,
//! regions, the wave pool, and INFO strings.
//!
//! Parsing only: no sample decoding and no synthesis. Borrowed slices point into the input
//! bytes. Chunks this parser does not model are kept as [`RawChunk`] entries (tag and body),
//! so nothing is dropped silently. A raw `LIST` entry keeps its list type at the start of its
//! body.
//!
//! Cross-references are checked during parsing: `colh` must match the instrument count, each
//! `ptbl` cue must name a wave, each `wlnk` table index must name a cue, and each region's
//! `rgn ` count must match `insh`.

const RIFF_HEADER_LEN: usize = 12;
const CHUNK_HEADER_LEN: usize = 8;
const LIST_TYPE_LEN: usize = 4;
const COLH_LEN: usize = 4;
const PTBL_MIN_LEN: usize = 8;
const CUE_LEN: usize = 4;
/// `rgnh` through `usKeyGroup`; `usLayer` follows only when the chunk is this long.
const RGNH_WITH_LAYER_LEN: usize = 14;
/// `wsmp` header through `cSampleLoops`.
const WSMP_HEADER_MIN_LEN: usize = 20;
/// `WLOOP` size, the smallest loop record that carries all its fields.
const WLOOP_MIN_LEN: usize = 16;
const ART1_MIN_LEN: usize = 8;
const CONNECTION_LEN: usize = 12;

#[derive(Debug, thiserror::Error)]
#[error("DLS bank at byte {at}: {reason}")]
pub struct DlsError {
    pub at: usize,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawChunk<'a> {
    pub tag: [u8; 4],
    pub body: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfoEntry<'a> {
    pub tag: [u8; 4],
    /// Bytes before the first NUL. The bank declares no code page, so the bytes are not decoded.
    pub text: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DlsBank<'a> {
    pub instruments: Vec<Instrument<'a>>,
    /// `ptbl` cue offsets, each relative to the first byte after the `wvpl` list type.
    pub pool_offsets: Vec<u32>,
    pub waves: Vec<Wave<'a>>,
    pub info: Vec<InfoEntry<'a>>,
    pub unknown: Vec<RawChunk<'a>>,
    /// Bytes the file holds beyond the declared RIFF size. Chunks are parsed to the end of the
    /// file. The shipped Ambient banks CityScape and LordaeronFall declare 22 bytes less than
    /// they hold: the RIFF size ends inside their last `INFO` sub-chunk.
    pub riff_size_shortfall: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instrument<'a> {
    pub locale: Locale,
    pub regions: Vec<Region<'a>>,
    /// Connection blocks from every `art1` chunk in the instrument's `lart` lists.
    pub articulators: Vec<Connection>,
    pub info: Vec<InfoEntry<'a>>,
    pub unknown: Vec<RawChunk<'a>>,
}

/// `insh` locale. `bank` and `program` are the MIDI bank and program the instrument answers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locale {
    pub bank: u32,
    pub program: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeU16 {
    pub low: u16,
    pub high: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region<'a> {
    pub key_range: RangeU16,
    pub velocity_range: RangeU16,
    pub options: u16,
    pub key_group: u16,
    /// Present only when the `rgnh` chunk carries `usLayer`.
    pub layer: Option<u16>,
    pub wave_links: Vec<WaveLink>,
    pub sample: Option<SampleControl>,
    pub articulators: Vec<Connection>,
    pub unknown: Vec<RawChunk<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveLink {
    pub options: u16,
    pub phase_group: u16,
    pub channel: u32,
    pub table_index: u32,
    /// Index into [`DlsBank::waves`] of the wave that `table_index` names.
    pub wave: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleControl {
    pub unity_note: u16,
    pub fine_tune: i16,
    pub attenuation: i32,
    pub options: u32,
    pub loops: Vec<SampleLoop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleLoop {
    pub kind: u32,
    pub start: u32,
    pub length: u32,
}

/// One articulation connection block (`CONNECTIONBLOCK`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connection {
    pub source: u16,
    pub control: u16,
    pub destination: u16,
    pub transform: u16,
    pub scale: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wave<'a> {
    pub format: WaveFormat,
    pub data: &'a [u8],
    pub info: Vec<InfoEntry<'a>>,
    pub unknown: Vec<RawChunk<'a>>,
}

/// The first 16 bytes of `WAVEFORMATEX`. Codec-specific extension bytes are not read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveFormat {
    pub format_tag: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
}

impl<'a> DlsBank<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, DlsError> {
        let bad = |at, reason| DlsError { at, reason };
        if bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"DLS ") {
            return Err(bad(0, "expected RIFF DLS"));
        }
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let riff_end = size
            .checked_add(CHUNK_HEADER_LEN)
            .filter(|&end| end <= bytes.len())
            .ok_or(bad(4, "declared RIFF size exceeds the file"))?;

        let (mut colh, mut ptbl, mut lins, mut wvpl) = (None, None, None, None);
        let mut info = Vec::new();
        let mut unknown = Vec::new();
        for chunk in read_chunks(&bytes[RIFF_HEADER_LEN..], RIFF_HEADER_LEN)? {
            match chunk.list()? {
                Some((kind, children)) => match &kind {
                    b"lins" => set_once(&mut lins, children, chunk.at, "duplicate lins")?,
                    b"wvpl" => {
                        set_once(&mut wvpl, (chunk.at, children), chunk.at, "duplicate wvpl")?
                    }
                    b"INFO" => info.extend(parse_info(&children)),
                    _ => unknown.push(chunk.raw()),
                },
                None => match &chunk.tag {
                    b"colh" => set_once(&mut colh, chunk, chunk.at, "duplicate colh")?,
                    b"ptbl" => set_once(&mut ptbl, chunk, chunk.at, "duplicate ptbl")?,
                    _ => unknown.push(chunk.raw()),
                },
            }
        }

        let colh = colh.ok_or(bad(RIFF_HEADER_LEN, "missing colh"))?;
        let instrument_count = parse_colh(colh)?;
        let pool_offsets = parse_ptbl(ptbl.ok_or(bad(RIFF_HEADER_LEN, "missing ptbl"))?)?;
        let (wvpl_at, wave_children) = wvpl.ok_or(bad(RIFF_HEADER_LEN, "missing wvpl"))?;
        let (waves, cue_waves) =
            parse_wave_pool(wvpl_at, wave_children, &pool_offsets, &mut unknown)?;
        let instrument_children = lins.ok_or(bad(RIFF_HEADER_LEN, "missing lins"))?;
        let instruments = parse_lins(instrument_children, &cue_waves, &mut unknown)?;
        if instruments.len() != instrument_count as usize {
            return Err(bad(colh.at, "colh count differs from the instrument count"));
        }
        Ok(Self {
            instruments,
            pool_offsets,
            waves,
            info,
            unknown,
            riff_size_shortfall: bytes.len() - riff_end,
        })
    }
}

fn set_once<T>(
    slot: &mut Option<T>,
    value: T,
    at: usize,
    reason: &'static str,
) -> Result<(), DlsError> {
    if slot.is_some() {
        return Err(DlsError { at, reason });
    }
    *slot = Some(value);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct Chunk<'a> {
    /// Offset of the chunk header in the file.
    at: usize,
    tag: [u8; 4],
    body: &'a [u8],
}

/// The list type and child chunks of a `LIST` chunk.
type ListParts<'a> = ([u8; 4], Vec<Chunk<'a>>);

impl<'a> Chunk<'a> {
    fn fields(self) -> Fields<'a> {
        Fields { chunk: self }
    }

    fn raw(self) -> RawChunk<'a> {
        RawChunk {
            tag: self.tag,
            body: self.body,
        }
    }

    /// The list type and child chunks of a `LIST` chunk, or `None` for any other chunk.
    fn list(self) -> Result<Option<ListParts<'a>>, DlsError> {
        if &self.tag != b"LIST" {
            return Ok(None);
        }
        let (kind, children) = self
            .body
            .split_first_chunk::<LIST_TYPE_LEN>()
            .ok_or(DlsError {
                at: self.at,
                reason: "LIST chunk without a list type",
            })?;
        let base = self.at + CHUNK_HEADER_LEN + LIST_TYPE_LEN;
        Ok(Some((*kind, read_chunks(children, base)?)))
    }
}

/// Little-endian field reads from one chunk body. Errors carry the file offset of the field.
#[derive(Debug, Clone, Copy)]
struct Fields<'a> {
    chunk: Chunk<'a>,
}

impl Fields<'_> {
    fn err(&self, at: usize, reason: &'static str) -> DlsError {
        DlsError {
            at: self.chunk.at + CHUNK_HEADER_LEN + at,
            reason,
        }
    }

    fn array<const N: usize>(&self, at: usize) -> Result<[u8; N], DlsError> {
        self.chunk
            .body
            .get(at..)
            .and_then(|rest| rest.get(..N))
            .and_then(|field| field.try_into().ok())
            .ok_or_else(|| self.err(at, "chunk too short for its fields"))
    }

    fn u16(&self, at: usize) -> Result<u16, DlsError> {
        Ok(u16::from_le_bytes(self.array(at)?))
    }

    fn i16(&self, at: usize) -> Result<i16, DlsError> {
        Ok(i16::from_le_bytes(self.array(at)?))
    }

    fn u32(&self, at: usize) -> Result<u32, DlsError> {
        Ok(u32::from_le_bytes(self.array(at)?))
    }

    fn i32(&self, at: usize) -> Result<i32, DlsError> {
        Ok(i32::from_le_bytes(self.array(at)?))
    }
}

/// Reads the chunks in `data`, whose first byte is at file offset `base`.
fn read_chunks(data: &[u8], base: usize) -> Result<Vec<Chunk<'_>>, DlsError> {
    let mut chunks = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        let rest = &data[at..];
        let header = rest.get(..CHUNK_HEADER_LEN).ok_or(DlsError {
            at: base + at,
            reason: "truncated chunk header",
        })?;
        let tag: [u8; 4] = header[..4].try_into().unwrap();
        let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let body_end = CHUNK_HEADER_LEN
            .checked_add(len)
            .filter(|&n| n <= rest.len())
            .ok_or(DlsError {
                at: base + at,
                reason: "chunk overruns its container",
            })?;
        let next = body_end
            .checked_add(len % 2)
            .filter(|&n| n <= rest.len())
            .ok_or(DlsError {
                at: base + at,
                reason: "missing chunk padding",
            })?;
        chunks.push(Chunk {
            at: base + at,
            tag,
            body: &rest[CHUNK_HEADER_LEN..body_end],
        });
        at += next;
    }
    Ok(chunks)
}

fn parse_colh(chunk: Chunk<'_>) -> Result<u32, DlsError> {
    if chunk.body.len() != COLH_LEN {
        return Err(DlsError {
            at: chunk.at,
            reason: "colh is not four bytes",
        });
    }
    chunk.fields().u32(0)
}

fn parse_ptbl(chunk: Chunk<'_>) -> Result<Vec<u32>, DlsError> {
    let f = chunk.fields();
    let header_len = f.u32(0)? as usize;
    if !(PTBL_MIN_LEN..=chunk.body.len()).contains(&header_len) {
        return Err(f.err(0, "ptbl header size out of range"));
    }
    let count = f.u32(4)? as usize;
    if count
        .checked_mul(CUE_LEN)
        .is_none_or(|cue_bytes| cue_bytes > chunk.body.len() - header_len)
    {
        return Err(f.err(4, "ptbl cue count exceeds the chunk"));
    }
    (0..count)
        .map(|index| f.u32(header_len + index * CUE_LEN))
        .collect()
}

/// Parses the `wvpl` children and maps each `ptbl` cue to a wave index.
fn parse_wave_pool<'a>(
    list_at: usize,
    children: Vec<Chunk<'a>>,
    cue_offsets: &[u32],
    unknown: &mut Vec<RawChunk<'a>>,
) -> Result<(Vec<Wave<'a>>, Vec<usize>), DlsError> {
    // Cue offsets count from the first child, after the `wvpl` list type.
    let base = list_at + CHUNK_HEADER_LEN + LIST_TYPE_LEN;
    let mut waves = Vec::new();
    let mut starts = Vec::new();
    for chunk in children {
        match chunk.list()? {
            Some((kind, kids)) if &kind == b"wave" => {
                starts.push(chunk.at - base);
                waves.push(parse_wave(chunk.at, kids)?);
            }
            _ => unknown.push(chunk.raw()),
        }
    }
    let cue_waves = cue_offsets
        .iter()
        .map(|&offset| {
            starts
                .binary_search(&(offset as usize))
                .map_err(|_| DlsError {
                    at: list_at,
                    reason: "pool cue names no wave",
                })
        })
        .collect::<Result<Vec<usize>, DlsError>>()?;
    Ok((waves, cue_waves))
}

fn parse_wave(at: usize, children: Vec<Chunk<'_>>) -> Result<Wave<'_>, DlsError> {
    let (mut format, mut data) = (None, None);
    let mut info = Vec::new();
    let mut unknown = Vec::new();
    for chunk in children {
        match chunk.list()? {
            Some((kind, kids)) if &kind == b"INFO" => info.extend(parse_info(&kids)),
            Some(_) => unknown.push(chunk.raw()),
            None => match &chunk.tag {
                b"fmt " => set_once(
                    &mut format,
                    parse_format(chunk)?,
                    chunk.at,
                    "duplicate wave fmt",
                )?,
                b"data" => set_once(&mut data, chunk.body, chunk.at, "duplicate wave data")?,
                _ => unknown.push(chunk.raw()),
            },
        }
    }
    Ok(Wave {
        format: format.ok_or(DlsError {
            at,
            reason: "wave without fmt",
        })?,
        data: data.ok_or(DlsError {
            at,
            reason: "wave without data",
        })?,
        info,
        unknown,
    })
}

fn parse_format(chunk: Chunk<'_>) -> Result<WaveFormat, DlsError> {
    let f = chunk.fields();
    Ok(WaveFormat {
        format_tag: f.u16(0)?,
        channels: f.u16(2)?,
        sample_rate: f.u32(4)?,
        average_bytes_per_second: f.u32(8)?,
        block_align: f.u16(12)?,
        bits_per_sample: f.u16(14)?,
    })
}

fn parse_lins<'a>(
    children: Vec<Chunk<'a>>,
    cue_waves: &[usize],
    unknown: &mut Vec<RawChunk<'a>>,
) -> Result<Vec<Instrument<'a>>, DlsError> {
    let mut instruments = Vec::new();
    for chunk in children {
        match chunk.list()? {
            Some((kind, kids)) if &kind == b"ins " => {
                instruments.push(parse_instrument(chunk.at, kids, cue_waves)?);
            }
            _ => unknown.push(chunk.raw()),
        }
    }
    Ok(instruments)
}

fn parse_instrument<'a>(
    at: usize,
    children: Vec<Chunk<'a>>,
    cue_waves: &[usize],
) -> Result<Instrument<'a>, DlsError> {
    let mut insh = None;
    let mut regions = Vec::new();
    let mut articulators = Vec::new();
    let mut info = Vec::new();
    let mut unknown = Vec::new();
    for chunk in children {
        match chunk.list()? {
            Some((kind, kids)) => match &kind {
                b"lrgn" => {
                    for region in kids {
                        match region.list()? {
                            Some((kind, rgn)) if &kind == b"rgn " => {
                                regions.push(parse_region(region.at, rgn, cue_waves)?);
                            }
                            _ => unknown.push(region.raw()),
                        }
                    }
                }
                b"lart" => articulators.extend(parse_lart(kids, &mut unknown)?),
                b"INFO" => info.extend(parse_info(&kids)),
                _ => unknown.push(chunk.raw()),
            },
            None => match &chunk.tag {
                b"insh" => set_once(&mut insh, chunk, chunk.at, "duplicate insh")?,
                _ => unknown.push(chunk.raw()),
            },
        }
    }
    let insh = insh.ok_or(DlsError {
        at,
        reason: "instrument without insh",
    })?;
    let f = insh.fields();
    let region_count = f.u32(0)?;
    let locale = Locale {
        bank: f.u32(4)?,
        program: f.u32(8)?,
    };
    if regions.len() != region_count as usize {
        return Err(DlsError {
            at,
            reason: "insh region count differs from the rgn lists",
        });
    }
    Ok(Instrument {
        locale,
        regions,
        articulators,
        info,
        unknown,
    })
}

fn parse_region<'a>(
    at: usize,
    children: Vec<Chunk<'a>>,
    cue_waves: &[usize],
) -> Result<Region<'a>, DlsError> {
    let mut header = None;
    let mut wave_links = Vec::new();
    let mut sample = None;
    let mut articulators = Vec::new();
    let mut unknown = Vec::new();
    for chunk in children {
        match chunk.list()? {
            Some((kind, kids)) if &kind == b"lart" => {
                articulators.extend(parse_lart(kids, &mut unknown)?);
            }
            Some(_) => unknown.push(chunk.raw()),
            None => match &chunk.tag {
                b"rgnh" => set_once(&mut header, chunk, chunk.at, "duplicate rgnh")?,
                b"wlnk" => wave_links.push(parse_wlnk(chunk, cue_waves)?),
                b"wsmp" => set_once(&mut sample, parse_wsmp(chunk)?, chunk.at, "duplicate wsmp")?,
                _ => unknown.push(chunk.raw()),
            },
        }
    }
    let header = header.ok_or(DlsError {
        at,
        reason: "region without rgnh",
    })?;
    let f = header.fields();
    let layer = if header.body.len() >= RGNH_WITH_LAYER_LEN {
        Some(f.u16(12)?)
    } else {
        None
    };
    Ok(Region {
        key_range: RangeU16 {
            low: f.u16(0)?,
            high: f.u16(2)?,
        },
        velocity_range: RangeU16 {
            low: f.u16(4)?,
            high: f.u16(6)?,
        },
        options: f.u16(8)?,
        key_group: f.u16(10)?,
        layer,
        wave_links,
        sample,
        articulators,
        unknown,
    })
}

fn parse_wlnk(chunk: Chunk<'_>, cue_waves: &[usize]) -> Result<WaveLink, DlsError> {
    let f = chunk.fields();
    let table_index = f.u32(8)?;
    let wave = usize::try_from(table_index)
        .ok()
        .and_then(|cue| cue_waves.get(cue).copied())
        .ok_or_else(|| f.err(8, "wave link names no pool cue"))?;
    Ok(WaveLink {
        options: f.u16(0)?,
        phase_group: f.u16(2)?,
        channel: f.u32(4)?,
        table_index,
        wave,
    })
}

fn parse_wsmp(chunk: Chunk<'_>) -> Result<SampleControl, DlsError> {
    let f = chunk.fields();
    let header_len = f.u32(0)? as usize;
    if !(WSMP_HEADER_MIN_LEN..=chunk.body.len()).contains(&header_len) {
        return Err(f.err(0, "wsmp header size out of range"));
    }
    let loop_count = f.u32(16)?;
    let mut loops = Vec::new();
    let mut at = header_len;
    for _ in 0..loop_count {
        let loop_len = f.u32(at)? as usize;
        if loop_len < WLOOP_MIN_LEN {
            return Err(f.err(at, "wsmp loop size is too small"));
        }
        loops.push(SampleLoop {
            kind: f.u32(at + 4)?,
            start: f.u32(at + 8)?,
            length: f.u32(at + 12)?,
        });
        at = at
            .checked_add(loop_len)
            .ok_or_else(|| f.err(at, "wsmp loop offset overflows"))?;
    }
    Ok(SampleControl {
        unity_note: f.u16(4)?,
        fine_tune: f.i16(6)?,
        attenuation: f.i32(8)?,
        options: f.u32(12)?,
        loops,
    })
}

fn parse_lart<'a>(
    children: Vec<Chunk<'a>>,
    unknown: &mut Vec<RawChunk<'a>>,
) -> Result<Vec<Connection>, DlsError> {
    let mut connections = Vec::new();
    for chunk in children {
        if &chunk.tag == b"art1" {
            connections.extend(parse_art1(chunk)?);
        } else {
            unknown.push(chunk.raw());
        }
    }
    Ok(connections)
}

fn parse_art1(chunk: Chunk<'_>) -> Result<Vec<Connection>, DlsError> {
    let f = chunk.fields();
    let header_len = f.u32(0)? as usize;
    if !(ART1_MIN_LEN..=chunk.body.len()).contains(&header_len) {
        return Err(f.err(0, "art1 header size out of range"));
    }
    let count = f.u32(4)? as usize;
    if count
        .checked_mul(CONNECTION_LEN)
        .is_none_or(|block_bytes| block_bytes > chunk.body.len() - header_len)
    {
        return Err(f.err(4, "art1 connection count exceeds the chunk"));
    }
    (0..count)
        .map(|index| {
            let at = header_len + index * CONNECTION_LEN;
            Ok(Connection {
                source: f.u16(at)?,
                control: f.u16(at + 2)?,
                destination: f.u16(at + 4)?,
                transform: f.u16(at + 6)?,
                scale: f.i32(at + 8)?,
            })
        })
        .collect()
}

fn parse_info<'a>(children: &[Chunk<'a>]) -> Vec<InfoEntry<'a>> {
    children
        .iter()
        .map(|chunk| InfoEntry {
            tag: chunk.tag,
            text: chunk
                .body
                .iter()
                .position(|&byte| byte == 0)
                .map_or(chunk.body, |end| &chunk.body[..end]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi_file::{ChannelMessage, Event, MidiFile};
    use pcx_mpq::MpqSet;
    use std::collections::{BTreeMap, BTreeSet};

    /// 16-byte PCM `fmt `: mono, 22050 Hz, 8-bit.
    const PCM_FMT: [u8; 16] = [1, 0, 1, 0, 0x22, 0x56, 0, 0, 0x22, 0x56, 0, 0, 1, 0, 8, 0];

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = tag.to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        if body.len() % 2 == 1 {
            out.push(0);
        }
        out
    }

    fn list(kind: &[u8; 4], children: &[u8]) -> Vec<u8> {
        let mut body = kind.to_vec();
        body.extend_from_slice(children);
        chunk(b"LIST", &body)
    }

    fn riff(children: &[u8]) -> Vec<u8> {
        let mut body = b"DLS ".to_vec();
        body.extend_from_slice(children);
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    fn wave(data: &[u8]) -> Vec<u8> {
        list(
            b"wave",
            &[chunk(b"fmt ", &PCM_FMT), chunk(b"data", data)].concat(),
        )
    }

    fn wlnk(table_index: u32) -> Vec<u8> {
        let mut body = vec![0u8; 12];
        body[8..12].copy_from_slice(&table_index.to_le_bytes());
        chunk(b"wlnk", &body)
    }

    /// `rgnh` with key 0..127, velocity 0..127, no layer.
    fn rgnh() -> Vec<u8> {
        chunk(b"rgnh", &[0, 0, 127, 0, 0, 0, 127, 0, 0, 0, 0, 0])
    }

    fn region(children: &[u8]) -> Vec<u8> {
        list(b"rgn ", children)
    }

    fn instrument(declared_regions: u32, program: u32, lrgn: &[u8], rest: &[u8]) -> Vec<u8> {
        let mut insh = declared_regions.to_le_bytes().to_vec();
        insh.extend_from_slice(&0u32.to_le_bytes());
        insh.extend_from_slice(&program.to_le_bytes());
        let children = [chunk(b"insh", &insh), list(b"lrgn", lrgn), rest.to_vec()].concat();
        list(b"ins ", &children)
    }

    /// A bank with `cues` into `waves` (the wave list children) and `lins` children.
    fn bank(colh_count: u32, cues: &[u32], waves: &[u8], lins: &[u8], extra: &[u8]) -> Vec<u8> {
        let mut ptbl = 8u32.to_le_bytes().to_vec();
        ptbl.extend_from_slice(&(cues.len() as u32).to_le_bytes());
        for cue in cues {
            ptbl.extend_from_slice(&cue.to_le_bytes());
        }
        let children = [
            chunk(b"colh", &colh_count.to_le_bytes()),
            chunk(b"ptbl", &ptbl),
            list(b"lins", lins),
            list(b"wvpl", waves),
            extra.to_vec(),
        ]
        .concat();
        riff(&children)
    }

    /// One instrument with one region on wave pool entry 0, in a bank with one wave.
    fn minimal_bank() -> Vec<u8> {
        let rgn = region(&[rgnh(), wlnk(0)].concat());
        let ins = instrument(1, 5, &rgn, &[]);
        bank(1, &[0], &wave(&[1, 2, 3, 4]), &ins, &[])
    }

    #[test]
    fn minimal_bank_parses_instrument_region_and_wave() {
        let bytes = minimal_bank();
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(parsed.instruments.len(), 1);
        let instrument = &parsed.instruments[0];
        assert_eq!(
            instrument.locale,
            Locale {
                bank: 0,
                program: 5
            }
        );
        let region = &instrument.regions[0];
        assert_eq!(region.key_range, RangeU16 { low: 0, high: 127 });
        assert_eq!(region.velocity_range, RangeU16 { low: 0, high: 127 });
        assert_eq!(region.layer, None);
        assert_eq!(region.sample, None);
        assert_eq!(region.wave_links.len(), 1);
        assert_eq!(region.wave_links[0].wave, 0);
        assert_eq!(region.wave_links[0].table_index, 0);
        assert_eq!(parsed.waves.len(), 1);
        assert_eq!(parsed.waves[0].data, &[1, 2, 3, 4]);
        assert_eq!(parsed.waves[0].format.sample_rate, 22050);
        assert_eq!(parsed.waves[0].format.bits_per_sample, 8);
        assert_eq!(parsed.pool_offsets, vec![0]);
    }

    #[test]
    fn cues_are_relative_to_the_first_child_after_the_wvpl_type() {
        let first = wave(&[1]);
        let second = wave(&[2]);
        let waves = [first.clone(), second.clone()].concat();
        // Table index 0 names the second wave, so the first cue is that wave's offset.
        let rgn = region(&[rgnh(), wlnk(0)].concat());
        let ins = instrument(1, 0, &rgn, &[]);
        let bytes = bank(1, &[first.len() as u32], &waves, &ins, &[]);
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(parsed.instruments[0].regions[0].wave_links[0].wave, 1);
        assert_eq!(parsed.waves[1].data, &[2]);
    }

    #[test]
    fn wsmp_with_loops_parses() {
        let mut wsmp = vec![20, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        wsmp[16] = 1; // one loop
        wsmp.extend_from_slice(&[16, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 8, 0, 0, 0]);
        let rgn = region(&[rgnh(), chunk(b"wsmp", &wsmp), wlnk(0)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        let parsed = DlsBank::parse(&bytes).unwrap();
        let sample = parsed.instruments[0].regions[0].sample.clone().unwrap();
        assert_eq!(sample.unity_note, 60);
        assert_eq!(
            sample.loops,
            vec![SampleLoop {
                kind: 0,
                start: 4,
                length: 8
            }]
        );
    }

    #[test]
    fn articulation_connections_parse() {
        let mut art1 = vec![8, 0, 0, 0, 1, 0, 0, 0];
        art1.extend_from_slice(&[1, 0, 2, 0, 3, 0, 4, 0]);
        art1.extend_from_slice(&(-96i32).to_le_bytes());
        let lart = list(b"lart", &chunk(b"art1", &art1));
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(0, 0, &[], &lart), &[]);
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(
            parsed.instruments[0].articulators,
            vec![Connection {
                source: 1,
                control: 2,
                destination: 3,
                transform: 4,
                scale: -96
            }]
        );
    }

    #[test]
    fn info_strings_and_unknown_chunks_are_kept() {
        let info = list(b"INFO", &chunk(b"INAM", b"Bank\0"));
        let rgn = region(&[rgnh(), wlnk(0), chunk(b"zzzz", &[9, 9, 9])].concat());
        let ins = instrument(1, 0, &rgn, &list(b"INFO", &chunk(b"ICMT", b"ok\0\0")));
        let mut extra = info;
        extra.extend_from_slice(&chunk(b"vers", &[0, 0, 1, 0, 0, 0, 0, 0]));
        let bytes = bank(1, &[0], &wave(&[1]), &ins, &extra);
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(
            parsed.info,
            vec![InfoEntry {
                tag: *b"INAM",
                text: b"Bank"
            }]
        );
        assert_eq!(
            parsed.unknown,
            vec![RawChunk {
                tag: *b"vers",
                body: &[0, 0, 1, 0, 0, 0, 0, 0]
            }]
        );
        assert_eq!(
            parsed.instruments[0].info,
            vec![InfoEntry {
                tag: *b"ICMT",
                text: b"ok"
            }]
        );
        assert_eq!(
            parsed.instruments[0].regions[0].unknown,
            vec![RawChunk {
                tag: *b"zzzz",
                body: &[9, 9, 9]
            }]
        );
    }

    #[test]
    fn layer_field_is_read_when_present() {
        let mut header = vec![0, 0, 127, 0, 0, 0, 127, 0, 0, 0, 0, 0];
        header.extend_from_slice(&3u16.to_le_bytes());
        let rgn = region(&[chunk(b"rgnh", &header), wlnk(0)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(parsed.instruments[0].regions[0].layer, Some(3));
    }

    #[test]
    fn empty_and_truncated_banks_are_refused() {
        assert_eq!(DlsBank::parse(&[]).unwrap_err().reason, "expected RIFF DLS");
        let bytes = minimal_bank();
        assert_eq!(
            DlsBank::parse(&bytes[..bytes.len() - 1])
                .unwrap_err()
                .reason,
            "declared RIFF size exceeds the file"
        );
        let mut wrong_form = bytes.clone();
        wrong_form[8..12].copy_from_slice(b"WAVE");
        assert_eq!(
            DlsBank::parse(&wrong_form).unwrap_err().reason,
            "expected RIFF DLS"
        );
    }

    #[test]
    fn riff_size_short_of_the_file_is_reported_and_chunks_parse_to_the_end() {
        assert_eq!(
            DlsBank::parse(&minimal_bank()).unwrap().riff_size_shortfall,
            0
        );
        let mut bytes = minimal_bank();
        let declared_end = bytes.len();
        let tail = list(b"INFO", &chunk(b"ISFT", b"Awave Studio v7.2\0"));
        bytes.extend_from_slice(&tail);
        bytes[4..8].copy_from_slice(&((declared_end - CHUNK_HEADER_LEN) as u32).to_le_bytes());
        let parsed = DlsBank::parse(&bytes).unwrap();
        assert_eq!(parsed.riff_size_shortfall, tail.len());
        assert_eq!(parsed.instruments.len(), 1);
        assert_eq!(
            parsed.info,
            vec![InfoEntry {
                tag: *b"ISFT",
                text: b"Awave Studio v7.2",
            }]
        );
    }

    #[test]
    fn missing_and_duplicate_top_level_chunks_are_refused() {
        let ins = instrument(0, 0, &[], &[]);
        let no_colh = riff(&[list(b"lins", &ins), list(b"wvpl", &[])].concat());
        assert_eq!(DlsBank::parse(&no_colh).unwrap_err().reason, "missing colh");
        let duplicate_lins = riff(
            &[
                chunk(b"colh", &[0, 0, 0, 0]),
                list(b"lins", &ins),
                list(b"lins", &ins),
            ]
            .concat(),
        );
        assert_eq!(
            DlsBank::parse(&duplicate_lins).unwrap_err().reason,
            "duplicate lins"
        );
        let dup = riff(&[chunk(b"colh", &[0, 0, 0, 0]), chunk(b"colh", &[0, 0, 0, 0])].concat());
        assert_eq!(DlsBank::parse(&dup).unwrap_err().reason, "duplicate colh");
    }

    #[test]
    fn instrument_and_region_counts_must_match() {
        let rgn = region(&[rgnh(), wlnk(0)].concat());
        let too_few = instrument(2, 0, &rgn, &[]);
        let bytes = bank(1, &[0], &wave(&[1]), &too_few, &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "insh region count differs from the rgn lists"
        );
        let ins = instrument(0, 0, &[], &[]);
        let bytes = bank(2, &[0], &wave(&[1]), &ins, &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "colh count differs from the instrument count"
        );
    }

    #[test]
    fn cross_references_must_resolve() {
        let rgn = region(&[rgnh(), wlnk(1)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "wave link names no pool cue"
        );
        let rgn = region(&[rgnh(), wlnk(0)].concat());
        let bytes = bank(1, &[7], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "pool cue names no wave"
        );
    }

    #[test]
    fn pool_table_size_is_checked_before_allocation() {
        let colh = chunk(b"colh", &0u32.to_le_bytes());
        let mut ptbl = 8u32.to_le_bytes().to_vec();
        ptbl.extend_from_slice(&u32::MAX.to_le_bytes());
        let bytes = riff(&[colh.clone(), chunk(b"ptbl", &ptbl)].concat());
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "ptbl cue count exceeds the chunk"
        );
        let mut short = 4u32.to_le_bytes().to_vec();
        short.extend_from_slice(&0u32.to_le_bytes());
        let bytes = riff(&[colh, chunk(b"ptbl", &short)].concat());
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "ptbl header size out of range"
        );
    }

    #[test]
    fn wave_must_have_format_and_data() {
        let no_data = list(b"wave", &chunk(b"fmt ", &PCM_FMT));
        let bytes = bank(0, &[0], &no_data, &[], &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "wave without data"
        );
        let short_fmt = list(
            b"wave",
            &[chunk(b"fmt ", &PCM_FMT[..15]), chunk(b"data", &[1])].concat(),
        );
        let bytes = bank(0, &[0], &short_fmt, &[], &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "chunk too short for its fields"
        );
    }

    #[test]
    fn wsmp_loop_overrun_and_bad_header_are_refused() {
        let mut wsmp = vec![20, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0];
        wsmp.extend_from_slice(&[16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let rgn = region(&[rgnh(), chunk(b"wsmp", &wsmp), wlnk(0)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "chunk too short for its fields"
        );

        let mut small_loop = vec![20, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        small_loop.extend_from_slice(&[8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let rgn = region(&[rgnh(), chunk(b"wsmp", &small_loop), wlnk(0)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "wsmp loop size is too small"
        );

        let bad_header = vec![4, 0, 0, 0, 60, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let rgn = region(&[rgnh(), chunk(b"wsmp", &bad_header), wlnk(0)].concat());
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(1, 0, &rgn, &[]), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "wsmp header size out of range"
        );
    }

    #[test]
    fn art1_overrun_is_refused() {
        let mut art1 = vec![8, 0, 0, 0, 2, 0, 0, 0];
        art1.extend_from_slice(&[0; 12]);
        let lart = list(b"lart", &chunk(b"art1", &art1));
        let bytes = bank(1, &[0], &wave(&[1]), &instrument(0, 0, &[], &lart), &[]);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "art1 connection count exceeds the chunk"
        );
    }

    #[test]
    fn chunk_framing_is_checked() {
        // A chunk whose declared length runs past its container.
        let mut overrun = b"zzzz".to_vec();
        overrun.extend_from_slice(&100u32.to_le_bytes());
        overrun.extend_from_slice(&[0; 4]);
        let bytes = riff(&overrun);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "chunk overruns its container"
        );
        // An odd-length chunk without its pad byte.
        let mut odd = b"zzzz".to_vec();
        odd.extend_from_slice(&1u32.to_le_bytes());
        odd.push(7);
        let bytes = riff(&odd);
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "missing chunk padding"
        );
        // A header cut short.
        let bytes = riff(b"zzz");
        assert_eq!(
            DlsBank::parse(&bytes).unwrap_err().reason,
            "truncated chunk header"
        );
    }

    #[test]
    fn every_proper_prefix_of_a_valid_bank_is_refused() {
        let bytes = minimal_bank();
        assert!(DlsBank::parse(&bytes).is_ok());
        for end in 0..bytes.len() {
            assert!(
                DlsBank::parse(&bytes[..end]).is_err(),
                "prefix of {end} bytes parsed"
            );
        }
    }

    #[test]
    fn mutated_banks_never_panic() {
        let original = minimal_bank();
        // xorshift64 with a fixed seed: deterministic byte mutations, no clock or host state.
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..4096 {
            let mut bytes = original.clone();
            let flips = 1 + (next() % 4) as usize;
            for _ in 0..flips {
                let index = (next() as usize) % bytes.len();
                bytes[index] = next() as u8;
            }
            let _ = DlsBank::parse(&bytes);
        }
    }

    /// Parses every `.mid` and `.dls` member of the local install and reports which MIDI
    /// bank/program pairs have no instrument in the `.dls` of the same folder.
    ///
    /// Requires an original install. Run with
    /// `PEACECRAFT3_INSTALL="<Warcraft III (Legacy) dir>" cargo test -p pcx-play dls_bank -- --ignored --nocapture`.
    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL=<Warcraft III (Legacy) directory> containing War3Local.mpq"]
    fn real_ambient_midi_and_dls_members_parse() {
        let install = std::env::var_os("PEACECRAFT3_INSTALL")
            .expect("set PEACECRAFT3_INSTALL to the Warcraft III (Legacy) directory");
        let mut archive =
            MpqSet::open_named(&install, &["War3Local.mpq"]).expect("open War3Local.mpq");
        let names: Vec<String> = archive
            .file_list()
            .expect("list War3Local.mpq")
            .into_iter()
            .filter(|name| {
                let lower = name.to_ascii_lowercase();
                lower.ends_with(".mid") || lower.ends_with(".dls")
            })
            .collect();
        let files: Vec<(String, Vec<u8>)> = names
            .into_iter()
            .map(|name| {
                let bytes = archive.extract(&name).expect("extract member");
                (name, bytes)
            })
            .collect();
        let midi_count = files
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().ends_with(".mid"))
            .count();
        let dls_count = files.len() - midi_count;
        println!("parsing {midi_count} .mid and {dls_count} .dls members");
        assert!(
            midi_count > 0 && dls_count > 0,
            "expected .mid and .dls members"
        );

        let folder = |name: &str| {
            name.rsplit_once(['\\', '/'])
                .map_or(String::new(), |(dir, _)| dir.to_ascii_lowercase())
        };
        let mut bank_locales: BTreeMap<String, BTreeSet<(u32, u32)>> = BTreeMap::new();
        for (name, bytes) in files
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().ends_with(".dls"))
        {
            let bank = DlsBank::parse(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
            println!(
                "{name}: {} instruments, {} waves, RIFF size short by {} bytes",
                bank.instruments.len(),
                bank.waves.len(),
                bank.riff_size_shortfall
            );
            let locales = bank_locales.entry(folder(name)).or_default();
            for instrument in &bank.instruments {
                locales.insert((instrument.locale.bank, instrument.locale.program));
            }
        }

        let mut missing = Vec::new();
        let mut checked = 0usize;
        for (name, bytes) in files
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().ends_with(".mid"))
        {
            let file = MidiFile::parse(bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
            println!(
                "{name}: format {:?}, {} tracks",
                file.format,
                file.tracks.len()
            );
            // Bank select is CC0 (MSB) and CC32 (LSB) per channel; both default to zero.
            let mut bank_select = [(0u8, 0u8); 16];
            let mut used = BTreeSet::new();
            for track in &file.tracks {
                for event in &track.events {
                    if let Event::Channel { channel, message } = event.event {
                        let slot = &mut bank_select[usize::from(channel)];
                        match message {
                            ChannelMessage::ControlChange {
                                controller: 0,
                                value,
                            } => slot.0 = value,
                            ChannelMessage::ControlChange {
                                controller: 32,
                                value,
                            } => slot.1 = value,
                            ChannelMessage::ProgramChange { program } => {
                                let bank = (u32::from(slot.0) << 8) | u32::from(slot.1);
                                used.insert((channel, bank, u32::from(program)));
                            }
                            _ => {}
                        }
                    }
                }
            }
            checked += used.len();
            let known = bank_locales.get(&folder(name));
            for (channel, bank, program) in used {
                let present = known.is_some_and(|set| set.contains(&(bank, program)));
                if !present {
                    missing.push(format!(
                        "{name}: channel {channel} bank {bank:#x} program {program}"
                    ));
                }
            }
        }
        if missing.is_empty() {
            println!(
                "checked {checked} MIDI bank/program pairs: each has an instrument in the .dls of its folder"
            );
        } else {
            println!(
                "MIDI bank/program pairs with no instrument in the folder .dls ({}):",
                missing.len()
            );
            for line in &missing {
                println!("  {line}");
            }
        }
    }
}
