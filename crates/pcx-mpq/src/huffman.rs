//! Storm Huffman decoding with weights imported from the user's PE image.
//! Native algorithm evidence and source identity are recorded in format-mpq.md.
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum HuffmanError {
    #[error("Huffman source {source_path}: {reason}")]
    Source { source_path: String, reason: String },
    #[error("Huffman stream is truncated or its tree is invalid")]
    Corrupt,
}

/// Runtime provenance; no original weights are compiled into this crate.
#[derive(Debug, Clone)]
pub struct HuffmanTable {
    pub source: PathBuf,
    pub table_va: u32,
    pub row_stride: usize,
    pub file_backed_bytes: usize,
    pub zero_filled_bytes: usize,
    rows: Vec<[u8; 256]>,
    /// ADPCM tables from the same source; `None` when the source has no unique run.
    pub adpcm: Option<AdpcmTables>,
}

/// ADPCM step and index-adjustment runs read from the codec source, with their VAs.
#[derive(Debug, Clone)]
pub struct AdpcmTables {
    pub steps_va: u32,
    pub index_va: u32,
    pub steps: Vec<u32>,
    pub index_adjust: Vec<i32>,
}

/// The step run is strictly increasing, starts above zero and ends at the 16-bit maximum.
const ADPCM_STEP_COUNT: usize = 89;
const ADPCM_INDEX_COUNT: usize = 32;
const ADPCM_STEP_MAX: u32 = i16::MAX as u32;
/// Index adjustments observed in the codec source lie in this inclusive range.
const ADPCM_INDEX_ADJUST_MIN: i32 = -1;
const ADPCM_INDEX_ADJUST_MAX: i32 = 8;

#[derive(Clone, Copy)]
struct Section {
    rva: usize,
    virtual_size: usize,
    raw: usize,
    raw_size: usize,
    code: bool,
}

fn word(bytes: &[u8], at: usize) -> Result<u16, HuffmanError> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .ok_or(HuffmanError::Corrupt)?
            .try_into()
            .unwrap(),
    ))
}
fn dword(bytes: &[u8], at: usize) -> Result<u32, HuffmanError> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or(HuffmanError::Corrupt)?
            .try_into()
            .unwrap(),
    ))
}

impl HuffmanTable {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, HuffmanError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| HuffmanError::Source {
            source_path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Self::from_pe(&bytes, path)
    }

    /// Derive the table operands from the x86 consumer, then map through PE sections.
    /// This input-derived instruction scan is a heuristic verified against the shipped
    /// source build, not a general disassembler. Missing/ambiguous consumers are refused.
    /// The documented fallback is an evidence-backed VA-to-section-offset probe of the
    /// caller's DLL (format-mpq.md); no guessed pointer is substituted on failure.
    /// The instruction formats are IMUL EAX,EAX,imm32 then LEA r32,[EAX+disp32]:
    /// ECX at 0x1500e153 in the evidenced Storm DLL, EAX at 0x44353f in the
    /// statically linked 1.29.2 client. The immediate and displacement stay local.
    pub fn from_pe(bytes: &[u8], source: impl AsRef<Path>) -> Result<Self, HuffmanError> {
        Self::parse(bytes, source.as_ref()).map_err(|e| HuffmanError::Source {
            source_path: source.as_ref().display().to_string(),
            reason: e.to_string(),
        })
    }

    fn parse(bytes: &[u8], source: &Path) -> Result<Self, HuffmanError> {
        if bytes.get(..2) != Some(b"MZ") {
            return Err(HuffmanError::Corrupt);
        }
        let pe = dword(bytes, 0x3c)? as usize;
        pe.checked_add(24).ok_or(HuffmanError::Corrupt)?;
        if bytes.get(pe..pe.checked_add(4).ok_or(HuffmanError::Corrupt)?) != Some(b"PE\0\0")
            || word(bytes, pe + 4)? != 0x14c
        {
            return Err(HuffmanError::Corrupt);
        }
        let optional = pe.checked_add(24).ok_or(HuffmanError::Corrupt)?;
        let optional_size = word(bytes, pe + 20)? as usize;
        if optional_size < 32 || word(bytes, optional)? != 0x10b {
            return Err(HuffmanError::Corrupt);
        }
        let image_base = dword(bytes, optional + 28)?;
        let sections_at = optional
            .checked_add(optional_size)
            .ok_or(HuffmanError::Corrupt)?;
        let sections_size = (word(bytes, pe + 6)? as usize) * 40;
        let section_bytes = bytes
            .get(
                sections_at
                    ..sections_at
                        .checked_add(sections_size)
                        .ok_or(HuffmanError::Corrupt)?,
            )
            .ok_or(HuffmanError::Corrupt)?;
        let mut sections = Vec::new();
        for header in section_bytes.as_chunks::<40>().0 {
            let section = Section {
                virtual_size: dword(header, 8)? as usize,
                rva: dword(header, 12)? as usize,
                raw_size: dword(header, 16)? as usize,
                raw: dword(header, 20)? as usize,
                code: dword(header, 36)? & 0x20 != 0,
            };
            bytes
                .get(
                    section.raw
                        ..section
                            .raw
                            .checked_add(section.raw_size)
                            .ok_or(HuffmanError::Corrupt)?,
                )
                .ok_or(HuffmanError::Corrupt)?;
            section
                .rva
                .checked_add(section.virtual_size.max(section.raw_size))
                .ok_or(HuffmanError::Corrupt)?;
            sections.push(section);
        }
        let mut operands = Vec::new();
        for section in sections.iter().filter(|s| s.code) {
            for code in bytes[section.raw..section.raw + section.raw_size].windows(12) {
                // LEA with mod=10, rm=EAX and any destination register.
                if code[..2] == [0x69, 0xc0] && code[6] == 0x8d && code[7] & 0xc7 == 0x80 {
                    let stride = dword(code, 2)? as usize;
                    let address = dword(code, 8)?;
                    // The evidenced consumer has 256 byte weights and two control symbols.
                    if stride == 256 + 2 && address >= image_base {
                        operands.push((stride, address));
                    }
                }
            }
        }
        if operands.len() != 1 {
            return Err(HuffmanError::Source {
                source_path: source.display().to_string(),
                reason: "no unique supported x86 Huffman table consumer".into(),
            });
        }
        let (row_stride, table_va) = operands[0];
        let table_rva = (table_va - image_base) as usize;
        let mut data = Vec::with_capacity(row_stride * 256);
        let (mut file_backed_bytes, mut zero_filled_bytes) = (0, 0);
        for offset in 0..row_stride * 256 {
            let rva = table_rva.checked_add(offset).ok_or(HuffmanError::Corrupt)?;
            let mut mapping = sections
                .iter()
                .filter(|s| rva >= s.rva && rva - s.rva < s.virtual_size.max(s.raw_size));
            let section = mapping.next().ok_or(HuffmanError::Corrupt)?;
            if mapping.next().is_some() {
                return Err(HuffmanError::Corrupt);
            }
            let within = rva - section.rva;
            if within < section.raw_size {
                data.push(bytes[section.raw + within]);
                file_backed_bytes += 1;
            } else {
                // PE VirtualSize > SizeOfRawData defines loader zero fill, not guessed weights.
                data.push(0);
                zero_filled_bytes += 1;
            }
        }
        let rows = data
            .chunks_exact(row_stride)
            .map(|row| row[..256].try_into().unwrap())
            .collect();
        let adpcm = find_adpcm_tables(bytes, &sections, image_base, source)?;
        Ok(Self {
            source: source.to_path_buf(),
            table_va,
            row_stride,
            file_backed_bytes,
            zero_filled_bytes,
            rows,
            adpcm,
        })
    }
}

/// Locate the ADPCM runs by shape in the file-backed bytes of every section.
///
/// The index run is the 32 signed dwords directly before the step run, in the same
/// section. Exactly one step run must match; zero matches yield `None`, so ADPCM
/// members refuse explicitly while Huffman members still decode. Several matches
/// are an error, since no ordering picks the right one.
fn find_adpcm_tables(
    bytes: &[u8],
    sections: &[Section],
    image_base: u32,
    source: &Path,
) -> Result<Option<AdpcmTables>, HuffmanError> {
    let ambiguous = || HuffmanError::Source {
        source_path: source.display().to_string(),
        reason: "several ADPCM step runs match the codec table shape".into(),
    };
    let index_bytes = ADPCM_INDEX_COUNT * 4;
    let step_bytes = ADPCM_STEP_COUNT * 4;
    let mut found: Option<(u32, u32, Vec<u32>, Vec<i32>)> = None;
    for section in sections {
        let raw = &bytes[section.raw..section.raw + section.raw_size];
        for at in index_bytes..=raw.len().saturating_sub(step_bytes) {
            let Some(steps) = read_steps(&raw[at..at + step_bytes]) else {
                continue;
            };
            let Some(index) = read_index(&raw[at - index_bytes..at]) else {
                continue;
            };
            let steps_va = image_base + (section.rva + at) as u32;
            let index_va = steps_va - index_bytes as u32;
            if found.is_some() {
                return Err(ambiguous());
            }
            found = Some((steps_va, index_va, steps, index));
        }
    }
    Ok(
        found.map(|(steps_va, index_va, steps, index_adjust)| AdpcmTables {
            steps_va,
            index_va,
            steps,
            index_adjust,
        }),
    )
}

/// `bytes` is exactly one step run. Rejects early so the scan allocates only on a match.
fn read_steps(bytes: &[u8]) -> Option<Vec<u32>> {
    let dword = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
    let first = dword(0);
    if first == 0 || dword(ADPCM_STEP_COUNT - 1) != ADPCM_STEP_MAX {
        return None;
    }
    let mut previous = first;
    for i in 1..ADPCM_STEP_COUNT {
        let step = dword(i);
        if step <= previous {
            return None;
        }
        previous = step;
    }
    Some((0..ADPCM_STEP_COUNT).map(dword).collect())
}

fn read_index(bytes: &[u8]) -> Option<Vec<i32>> {
    let index: Vec<i32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|dword| i32::from_le_bytes(*dword))
        .collect();
    index
        .iter()
        .all(|&adjust| (ADPCM_INDEX_ADJUST_MIN..=ADPCM_INDEX_ADJUST_MAX).contains(&adjust))
        .then_some(index)
}

#[derive(Default, Clone, Copy)]
struct Node {
    next: usize,
    prev: usize,
    symbol: usize,
    weight: u32,
    parent: usize,
    low: usize,
}

struct Tree {
    // Index zero replaces Storm's intrusive-list sentinel. All links are indices.
    nodes: Vec<Node>,
    symbols: [usize; 258],
}

impl Tree {
    fn new(weights: &[u8; 256]) -> Result<Self, HuffmanError> {
        let mut tree = Self {
            nodes: vec![Node::default()],
            symbols: [0; 258],
        };
        let mut maximum = 0;
        for (symbol, &weight) in weights.iter().enumerate().filter(|(_, w)| **w != 0) {
            let node = tree.allocate(Some(symbol), weight as u32, 0)?;
            tree.insert_after(0, node);
            if (weight as u32) < maximum {
                let place = tree.find_weight(tree.nodes[0].prev, weight as u32);
                tree.remove(node);
                tree.insert_after(place, node);
            } else {
                maximum = weight as u32;
            }
        }
        for symbol in [0x100, 0x101] {
            let node = tree.allocate(Some(symbol), 1, 0)?;
            tree.insert_after(tree.nodes[0].prev, node);
        }
        let mut low = tree.nodes[0].prev;
        while low != 0 && tree.nodes[low].prev != 0 {
            let high = tree.nodes[low].prev;
            let weight = tree.nodes[low].weight + tree.nodes[high].weight;
            let parent = tree.allocate(None, weight, 0)?;
            tree.nodes[parent].low = low;
            tree.nodes[low].parent = parent;
            tree.nodes[high].parent = parent;
            tree.insert_after(0, parent);
            if weight < maximum {
                let place = tree.find_weight(tree.nodes[high].prev, weight);
                tree.remove(parent);
                tree.insert_after(place, parent);
            } else {
                maximum = weight;
            }
            low = tree.nodes[high].prev;
        }
        Ok(tree)
    }

    fn allocate(
        &mut self,
        symbol: Option<usize>,
        weight: u32,
        parent: usize,
    ) -> Result<usize, HuffmanError> {
        // FUN_1500e7d0 destroys exactly 0x203 nodes; two children per insertion.
        if self.nodes.len() > 0x203 {
            return Err(HuffmanError::Corrupt);
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            symbol: symbol.unwrap_or(0),
            weight,
            parent,
            ..Node::default()
        });
        if let Some(symbol) = symbol {
            self.symbols[symbol] = id;
        }
        Ok(id)
    }
    fn remove(&mut self, node: usize) {
        let Node { next, prev, .. } = self.nodes[node];
        self.nodes[prev].next = next;
        self.nodes[next].prev = prev;
    }
    fn insert_after(&mut self, place: usize, node: usize) {
        let next = self.nodes[place].next;
        self.nodes[node].prev = place;
        self.nodes[node].next = next;
        self.nodes[next].prev = node;
        self.nodes[place].next = node;
    }
    fn find_weight(&self, mut node: usize, weight: u32) -> usize {
        while node != 0 && weight > self.nodes[node].weight {
            node = self.nodes[node].prev;
        }
        node
    }

    // FUN_1500e4c0: increment ancestors, exchange list positions and parent links.
    fn update(&mut self, mut node: usize) -> Result<(), HuffmanError> {
        let mut steps = 0;
        while node != 0 {
            steps += 1;
            if steps > self.nodes.len() {
                return Err(HuffmanError::Corrupt);
            }
            self.nodes[node].weight = self.nodes[node]
                .weight
                .checked_add(1)
                .ok_or(HuffmanError::Corrupt)?;
            let mut leader = node;
            let mut place = self.nodes[node].prev;
            while place != 0 && self.nodes[node].weight > self.nodes[place].weight {
                leader = place;
                place = self.nodes[place].prev;
            }
            if leader != node {
                let left_parent = self.nodes[node].parent;
                let right_parent = self.nodes[leader].parent;
                if left_parent == 0 || right_parent == 0 {
                    return Err(HuffmanError::Corrupt);
                }
                self.remove(leader);
                self.insert_after(node, leader);
                self.remove(node);
                self.insert_after(place, node);
                let right_low = self.nodes[right_parent].low;
                if self.nodes[left_parent].low == node {
                    self.nodes[left_parent].low = leader;
                }
                if right_low == leader {
                    self.nodes[right_parent].low = node;
                }
                self.nodes[node].parent = right_parent;
                self.nodes[leader].parent = left_parent;
            }
            node = self.nodes[node].parent;
        }
        Ok(())
    }

    // FUN_1500e630: split the list's lightest leaf into its old symbol and the new one.
    fn insert(&mut self, symbol: usize) -> Result<(), HuffmanError> {
        if self.symbols[symbol] != 0 {
            return Err(HuffmanError::Corrupt);
        }
        let parent = self.nodes[0].prev;
        if parent == 0 || self.nodes[parent].low != 0 {
            return Err(HuffmanError::Corrupt);
        }
        let old = self.nodes[parent];
        let high = self.allocate(Some(old.symbol), old.weight, parent)?;
        self.insert_after(self.nodes[0].prev, high);
        let low = self.allocate(Some(symbol), 0, parent)?;
        self.insert_after(self.nodes[0].prev, low);
        self.nodes[parent].low = low;
        self.update(low)
    }

    // FUN_1500df20 without its optional seven-bit decode cache.
    fn symbol(&self, bits: &mut Bits<'_>) -> Result<usize, HuffmanError> {
        let mut node = self.nodes[0].next;
        for _ in 0..self.nodes.len() {
            if node == 0 {
                return Err(HuffmanError::Corrupt);
            }
            if self.nodes[node].low == 0 {
                return Ok(self.nodes[node].symbol);
            }
            node = self.nodes[node].low;
            if bits.take(1)? != 0 {
                node = self.nodes[node].prev;
            }
        }
        Err(HuffmanError::Corrupt)
    }
}

struct Bits<'a> {
    bytes: &'a [u8],
    buffered: u16,
    available: usize,
}
impl Bits<'_> {
    fn take(&mut self, count: usize) -> Result<usize, HuffmanError> {
        assert!(count <= 8);
        // Keep unconsumed bits so tree traversal loads each input byte only once.
        if self.available < count {
            let (&byte, remaining) = self.bytes.split_first().ok_or(HuffmanError::Corrupt)?;
            self.buffered |= u16::from(byte) << self.available;
            self.available += 8;
            self.bytes = remaining;
        }
        let value = self.buffered & ((1 << count) - 1);
        self.buffered >>= count;
        self.available -= count;
        Ok(usize::from(value))
    }
}

/// FUN_1500e870; bounded LSB reader preserves output behavior without native overreads.
pub fn decompress(
    body: &[u8],
    out: &mut [u8],
    table: &HuffmanTable,
) -> Result<usize, HuffmanError> {
    if out.is_empty() {
        return Ok(0);
    }
    let mut bits = Bits {
        bytes: body,
        buffered: 0,
        available: 0,
    };
    let row = bits.take(8)?;
    let mut tree = Tree::new(&table.rows[row])?;
    let adaptive = row == 0;
    let mut written = 0;
    while written < out.len() {
        let mut symbol = tree.symbol(&mut bits)?;
        if symbol == 0x101 {
            symbol = bits.take(8)?;
            tree.insert(symbol)?;
            if !adaptive {
                tree.update(tree.symbols[symbol])?;
            }
        }
        if symbol == 0x100 {
            break;
        }
        out[written] = symbol as u8;
        written += 1;
        if written < out.len() && adaptive {
            tree.update(tree.symbols[symbol])?;
        }
    }
    Ok(written)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn synthetic_table(row: usize, weights: [u8; 256]) -> HuffmanTable {
        let mut rows = vec![[0; 256]; 256];
        rows[row] = weights;
        HuffmanTable {
            source: "synthetic".into(),
            table_va: 0,
            row_stride: 258,
            file_backed_bytes: 0,
            zero_filled_bytes: 0,
            rows,
            adpcm: None,
        }
    }

    /// Synthetic run pair: strictly increasing steps ending at the 16-bit maximum, and
    /// index adjustments inside the accepted range. Expected decoder outputs are computed
    /// from these values by an independent script, not from the production tables.
    pub(crate) fn fixture_steps() -> [u32; ADPCM_STEP_COUNT] {
        std::array::from_fn(|i| {
            if i == ADPCM_STEP_COUNT - 1 {
                ADPCM_STEP_MAX
            } else {
                7 + 367 * i as u32
            }
        })
    }
    pub(crate) fn fixture_index_adjust() -> [i32; ADPCM_INDEX_COUNT] {
        std::array::from_fn(|i| if i % 2 == 0 { -1 } else { (i % 9) as i32 })
    }
    const FIXTURE_INDEX_VA: u32 = 0x420000;

    pub(crate) fn fixture_adpcm() -> AdpcmTables {
        AdpcmTables {
            steps_va: FIXTURE_INDEX_VA + 128,
            index_va: FIXTURE_INDEX_VA,
            steps: fixture_steps().to_vec(),
            index_adjust: fixture_index_adjust().to_vec(),
        }
    }

    /// Two x86 consumers, the Huffman operands, and an ADPCM run pair in a third section.
    pub(crate) fn synthetic_pe() -> Vec<u8> {
        let mut bytes = vec![0; 0x800];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        bytes[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&3u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());
        bytes[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        bytes[0xb4..0xb8].copy_from_slice(&0x400000u32.to_le_bytes());
        for (at, rva, raw, raw_size, virtual_size, flags) in [
            (0x178, 0x1000u32, 0x200u32, 0x20u32, 0x20u32, 0x20u32),
            (0x1a0, 0x2000, 0x220, 258, 258 * 256, 0),
            (0x1c8, 0x20000, 0x400, 0x400, 0x400, 0x40),
        ] {
            for (offset, value) in [
                (8, virtual_size),
                (12, rva),
                (16, raw_size),
                (20, raw),
                (36, flags),
            ] {
                bytes[at + offset..at + offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes[0x200..0x202].copy_from_slice(&[0x69, 0xc0]);
        bytes[0x202..0x206].copy_from_slice(&258u32.to_le_bytes());
        bytes[0x206..0x208].copy_from_slice(&[0x8d, 0x88]);
        bytes[0x208..0x20c].copy_from_slice(&0x402000u32.to_le_bytes());
        bytes[0x220 + 65] = 4;
        for (i, adjust) in fixture_index_adjust().iter().enumerate() {
            bytes[0x400 + i * 4..0x404 + i * 4].copy_from_slice(&adjust.to_le_bytes());
        }
        for (i, step) in fixture_steps().iter().enumerate() {
            bytes[0x480 + i * 4..0x484 + i * 4].copy_from_slice(&step.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn pe_operands_and_sections_define_runtime_weights() {
        let bytes = synthetic_pe();
        let table = HuffmanTable::from_pe(&bytes, "fixture.dll").unwrap();
        assert_eq!(table.table_va, 0x402000);
        assert_eq!(table.row_stride, 258);
        assert_eq!(table.file_backed_bytes, 258);
        assert_eq!(table.zero_filled_bytes, 258 * 255);
        assert_eq!(table.rows[0][65], 4);
        assert_eq!(table.rows[1], [0; 256]);
        let adpcm = table.adpcm.as_ref().expect("ADPCM run pair");
        assert_eq!(adpcm.index_va, 0x420000);
        assert_eq!(adpcm.steps_va, 0x420080);
        assert_eq!(adpcm.steps, fixture_steps());
        assert_eq!(adpcm.index_adjust, fixture_index_adjust());
    }

    /// Replace the third section's run pair with `bytes` written at `at` (file offset).
    fn with_run_at(mut bytes: Vec<u8>, at: usize) -> Vec<u8> {
        for (i, adjust) in fixture_index_adjust().iter().enumerate() {
            bytes[at + i * 4..at + i * 4 + 4].copy_from_slice(&adjust.to_le_bytes());
        }
        for (i, step) in fixture_steps().iter().enumerate() {
            bytes[at + 128 + i * 4..at + 132 + i * 4].copy_from_slice(&step.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn adpcm_runs_are_absent_or_ambiguous_by_shape() {
        // No run pair: Huffman still loads, ADPCM stays unavailable.
        let absent = synthetic_pe();
        let mut without = absent.clone();
        without[0x400..0x400 + 128 + 356].fill(0);
        assert!(
            HuffmanTable::from_pe(&without, "absent.dll")
                .unwrap()
                .adpcm
                .is_none()
        );
        // An index adjustment outside the observed range is not a run.
        let mut out_of_range = absent.clone();
        out_of_range[0x400..0x404].copy_from_slice(&9i32.to_le_bytes());
        assert!(
            HuffmanTable::from_pe(&out_of_range, "range.dll")
                .unwrap()
                .adpcm
                .is_none()
        );
        // A step run that is not strictly increasing is not a run.
        let mut flat = absent.clone();
        flat[0x480 + 4..0x484 + 4].copy_from_slice(&7u32.to_le_bytes());
        assert!(
            HuffmanTable::from_pe(&flat, "flat.dll")
                .unwrap()
                .adpcm
                .is_none()
        );
        // Two identical run pairs cannot be ordered, so the source is refused.
        let twice = with_run_at(absent, 0x600);
        assert!(matches!(
            HuffmanTable::from_pe(&twice, "twice.dll"),
            Err(HuffmanError::Source { reason, .. }) if reason.contains("several ADPCM")
        ));
    }

    #[test]
    fn malformed_pe_and_unmapped_or_ambiguous_operands_are_refused() {
        let bytes = synthetic_pe();
        for length in [0, 64, 0x80, 0x200, bytes.len() - 1] {
            assert!(matches!(
                HuffmanTable::from_pe(&bytes[..length], "truncated.dll"),
                Err(HuffmanError::Source { .. })
            ));
        }
        let mut gap = bytes.clone();
        gap[0x208..0x20c].copy_from_slice(&0x600000u32.to_le_bytes());
        assert!(HuffmanTable::from_pe(&gap, "gap.dll").is_err());
        let mut ambiguous = bytes.clone();
        ambiguous.copy_within(0x200..0x20c, 0x20c);
        assert!(HuffmanTable::from_pe(&ambiguous, "ambiguous.dll").is_err());
        // The statically linked client loads the row into EAX instead of ECX.
        let mut eax = bytes.clone();
        eax[0x207] = 0x80;
        assert_eq!(
            HuffmanTable::from_pe(&eax, "eax.exe").unwrap().table_va,
            0x402000
        );
        // Other addressing modes are not the evidenced consumer.
        let mut other = bytes;
        other[0x207] = 0x81;
        assert!(HuffmanTable::from_pe(&other, "other.dll").is_err());
    }

    #[test]
    fn buffered_reads_match_bitwise_reads_at_byte_boundaries_and_truncation() {
        for seed in 0..=u8::MAX {
            let body = [seed, seed.rotate_left(3), !seed, seed.wrapping_mul(37)];
            for length in 0..=body.len() {
                for offset in 0..8 {
                    for width in 1..=8 {
                        let mut bits = Bits {
                            bytes: &body[..length],
                            buffered: 0,
                            available: 0,
                        };
                        let mut position = 0;
                        for count in [offset, width, 1, 8, width, 1, 8] {
                            let expected = (0..count).try_fold(0, |value, shift| {
                                let byte = body[..length].get(position / 8)?;
                                let bit = (byte >> (position % 8)) & 1;
                                position += 1;
                                Some(value | (usize::from(bit) << shift))
                            });
                            assert_eq!(bits.take(count).ok(), expected);
                            if expected.is_none() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn equal_weight_order_termination_and_output_limit() {
        let mut weights = [0; 256];
        weights[b'A' as usize] = 1;
        weights[b'B' as usize] = 1;
        let table = synthetic_table(1, weights);
        // Native tie order yields A=bits 1,0; B=1,1; end=0,1 (LSB first).
        let mut out = [0; 8];
        assert_eq!(decompress(&[1, 0x2d], &mut out, &table).unwrap(), 2);
        assert_eq!(&out[..2], b"AB");
        assert_eq!(decompress(&[1, 0x2d], &mut out[..1], &table).unwrap(), 1);
        assert!(decompress(&[1], &mut out, &table).is_err());
    }

    #[test]
    fn adaptive_escape_insertion_and_weight_exchange() {
        let table = synthetic_table(0, [0; 256]);
        // Escape=0, literal A (8 bits), repeated A=0, then end=0,0 after exchanges.
        let mut out = [0; 8];
        assert_eq!(decompress(&[0, 0x82, 0], &mut out, &table).unwrap(), 2);
        assert_eq!(&out[..2], b"AA");
    }

    #[test]
    fn huffman_then_adpcm_composes_in_dispatch_order() {
        let mut weights = [0; 256];
        weights[0] = 1;
        weights[1] = 1;
        let mut table = synthetic_table(1, weights);
        table.adpcm = Some(fixture_adpcm());
        let mut body = vec![0, 1, 0, 0];
        body.extend([0; 16]);
        let mut bits = vec![1u8];
        for (position, bit) in (8..).zip(body.iter().flat_map(|byte| [1, *byte]).chain([0, 1])) {
            if position / 8 == bits.len() {
                bits.push(0);
            }
            bits[position / 8] |= bit << (position % 8);
        }
        let mut packed = vec![0x41];
        packed.extend(bits);
        let mut expected = [0; 34];
        let mut adpcm = vec![0x40];
        adpcm.extend(body);
        crate::payload::decompress_sector_with_huffman(&adpcm, &mut expected, Some(&table))
            .unwrap();
        let mut actual = [0; 34];
        assert_eq!(
            crate::payload::decompress_sector_with_huffman(&packed, &mut actual, Some(&table))
                .unwrap(),
            34
        );
        assert_eq!(actual, expected);
        assert!(matches!(
            crate::payload::decompress_sector(&packed, &mut actual),
            Err(crate::payload::PayloadError::UnsupportedCompression { mask: 0x41 })
        ));
    }
}
