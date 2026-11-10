//! `war3campaign.w3f`, the campaign info file inside a `.w3n` campaign container.
//!
//! # Where the file lives
//!
//! A `.w3n` is the `HM3W` container that [`crate::Map`] parses: a 0x200-byte
//! header, an embedded MPQ, and an optional trailer. `war3campaign.w3f` is a
//! member of that embedded archive:
//!
//! ```text
//! let w3f = Map::parse(&container)?.extract(CAMPAIGN_INFO_NAME)?;
//! let info = CampaignInfo::parse(&w3f)?;
//! ```
//!
//! # What is measured
//!
//! One `.w3n` ships in the 1.28c install: `DemoCampaign.w3n`, a member of
//! `War3x.mpq`, byte-identical to the loose copy under `Campaigns/`. Its
//! `war3campaign.w3f` is 303 bytes. The Legacy 1.29 tree ships no `.w3n`; a
//! listing of every archive and of `Maps/` found none. The sample is one file,
//! so nothing below is proven across campaigns.
//!
//! The walk consumes the sample exactly, 303 of 303 bytes, and both counts land
//! their lists on the bytes that follow them. That is the framing evidence. The
//! meaning of each field is not established.
//!
//! # Layout, version 1
//!
//! Strings are NUL-terminated. `TRIGSTR_*` values are kept as the raw
//! references the file holds; they are not resolved against `war3campaign.wts`.
//!
//! ```text
//! offset  size  field
//! 0x00    4     version word                       = 1
//! 0x04    4     header word 0                      = 158
//! 0x08    4     header word 1                      = 6030
//! 0x0c    str   strings[0]                         = "TRIGSTR_001"
//!         str   strings[1]                         = "TRIGSTR_002"
//!         str   strings[2]                         = "TRIGSTR_004"
//!         str   strings[3]                         = "TRIGSTR_003"
//! 0x3c    4     words_after_strings[0]             = 1
//! 0x40    4     words_after_strings[1]             = 0
//! 0x44    str   string_after_words                 = ""
//! 0x45    str   texture_path                       = "UI\Widgets\Glues\Minimap-CustomCampaign-Orc"
//! 0x71    13    opaque                             = all zero
//! 0x7e    4     floats[0] (f32)                    = 2000.0
//! 0x82    4     floats[1] (f32)                    = 0.5
//! 0x86    4     bytes_after_floats                 = 4d 33 00 ff
//! 0x8a    4     word_after_bytes                   = 1
//! 0x8e    4     entry count                        = 3
//!               entry x count:
//!                 4     word                       = 0x0012f8e5, 0x0012f8e4, 0x0012f8e4
//!                 str   strings[0..3]              = TRIGSTR refs, then a ".w3m" path
//! after   4     trailing entry count               = 3
//!               trailing entry x count:
//!                 str   strings[0]                 = ""
//!                 str   strings[1]                 = ".w3m" path
//! ```
//!
//! The 13-byte opaque block, the 4-byte blocks, and both list widths are
//! measured on this one sample. A file whose layout differs in any width fails
//! the walk and is refused; no offset is re-derived.
//!
//! # Not established
//!
//! No offset above is attributed to a field of the campaign editor. The brief's
//! list (file, campaign, and editor version; name; difficulty; author;
//! description; flags; background; minimap index; ambient sound; terrain and fog;
//! race; per-map buttons) is not matched to any position. The entry list is the
//! likely per-map button list. That is unverified.
//!
//! Native evidence: `game.dll` in the 1.28c tree loads the `.w3f` literal at
//! VA `0x6f981144` into `edx` at `0x6f7388c7`. The field walk behind that call is
//! not decoded.
//!
//! # What is refused
//!
//! * a version word other than [`VERSION`];
//! * a string with no NUL before the end of the file, or one that is not UTF-8;
//! * a declared count that cannot fit in the bytes left, checked before any
//!   allocation;
//! * bytes left over after the last list.

/// Name of the campaign info member inside a `.w3n` archive.
pub const CAMPAIGN_INFO_NAME: &str = "war3campaign.w3f";

/// The only version word this walk accepts. Measured on the one sample.
pub const VERSION: u32 = 1;

/// Bytes in the opaque block at `+0x71`. Measured on the one sample.
pub const OPAQUE_BLOCK_LEN: usize = 13;

/// Bytes in the block at `+0x86`. Measured on the one sample.
pub const BYTE_QUAD_LEN: usize = 4;

const WORD_BYTES: usize = 4;
const EMPTY_STRING_BYTES: usize = 1;
/// Smallest entry: a word and three empty strings.
const ENTRY_MIN_BYTES: usize = WORD_BYTES + 3 * EMPTY_STRING_BYTES;
/// Smallest trailing entry: two empty strings.
const TRAILING_MIN_BYTES: usize = 2 * EMPTY_STRING_BYTES;

/// Why a `war3campaign.w3f` payload was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CampaignInfoError {
    #[error("unsupported campaign info version {0}; only 1 is measured")]
    UnsupportedVersion(u32),
    #[error("ran out of bytes reading {what} at +{at:#x}: {have} left, {need} needed")]
    Truncated {
        what: &'static str,
        at: usize,
        have: usize,
        need: usize,
    },
    #[error("string at +{at:#x} reaches the end of the file without a NUL")]
    UnterminatedString { at: usize },
    #[error("string at +{at:#x} is not UTF-8")]
    InvalidUtf8 { at: usize },
    #[error("declared {what} count exceeds the bytes remaining at +{at:#x}: {have} left")]
    CountExceedsBytes {
        what: &'static str,
        at: usize,
        have: usize,
    },
    #[error("{have} bytes left over after the last list at +{at:#x}")]
    TrailingBytes { at: usize, have: usize },
}

/// One entry of the first list, count at `+0x8e`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignEntry {
    /// The word before the strings.
    pub word: u32,
    /// Two TRIGSTR-style references, then a `.w3m` path on the sample.
    pub strings: [String; 3],
}

/// One entry of the trailing list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailingEntry {
    /// An empty string, then a `.w3m` path on the sample.
    pub strings: [String; 2],
}

/// A parsed `war3campaign.w3f`. Every field is positional; see the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct CampaignInfo {
    /// `+0x00`, gates the layout.
    pub version: u32,
    /// `+0x04` and `+0x08`.
    pub header_words: [u32; 2],
    /// `+0x0c`, raw TRIGSTR references.
    pub strings: [String; 4],
    /// `+0x3c` and `+0x40`.
    pub words_after_strings: [u32; 2],
    /// `+0x44`.
    pub string_after_words: String,
    /// `+0x45`, a texture path as written.
    pub texture_path: String,
    /// `+0x71`.
    pub opaque: [u8; OPAQUE_BLOCK_LEN],
    /// `+0x7e` and `+0x82`.
    pub floats: [f32; 2],
    /// `+0x86`.
    pub bytes_after_floats: [u8; BYTE_QUAD_LEN],
    /// `+0x8a`.
    pub word_after_bytes: u32,
    pub entries: Vec<CampaignEntry>,
    pub trailing_entries: Vec<TrailingEntry>,
}

/// A cursor over the `.w3f` bytes. Every read checks its bounds first, and a
/// failed read leaves the position unchanged.
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn left(&self) -> usize {
        self.data.len().saturating_sub(self.at)
    }

    fn array<const N: usize>(&mut self, what: &'static str) -> Result<[u8; N], CampaignInfoError> {
        let Some(chunk) = self
            .data
            .get(self.at..)
            .and_then(|rest| rest.first_chunk::<N>())
        else {
            return Err(CampaignInfoError::Truncated {
                what,
                at: self.at,
                have: self.left(),
                need: N,
            });
        };
        self.at += N;
        Ok(*chunk)
    }

    fn u32(&mut self, what: &'static str) -> Result<u32, CampaignInfoError> {
        Ok(u32::from_le_bytes(self.array::<WORD_BYTES>(what)?))
    }

    fn f32(&mut self, what: &'static str) -> Result<f32, CampaignInfoError> {
        Ok(f32::from_bits(self.u32(what)?))
    }

    /// Reads a count word and refuses it unless `min_record_bytes` per record
    /// fit in the bytes that follow it.
    fn count(
        &mut self,
        what: &'static str,
        min_record_bytes: usize,
    ) -> Result<usize, CampaignInfoError> {
        let word = self.u32(what)?;
        let have = self.left();
        let count = usize::try_from(word).unwrap_or(usize::MAX);
        if count <= have / min_record_bytes {
            Ok(count)
        } else {
            Err(CampaignInfoError::CountExceedsBytes {
                what,
                at: self.at,
                have,
            })
        }
    }

    fn string(&mut self) -> Result<String, CampaignInfoError> {
        let start = self.at;
        let rest = self.data.get(start..).unwrap_or_default();
        let nul = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or(CampaignInfoError::UnterminatedString { at: start })?;
        let text = std::str::from_utf8(&rest[..nul])
            .map_err(|_| CampaignInfoError::InvalidUtf8 { at: start })?;
        self.at = start + nul + 1;
        Ok(text.to_owned())
    }

    fn finish(&self) -> Result<(), CampaignInfoError> {
        match self.left() {
            0 => Ok(()),
            have => Err(CampaignInfoError::TrailingBytes { at: self.at, have }),
        }
    }
}

impl CampaignInfo {
    /// Parse a `war3campaign.w3f` payload. Refuses any version other than
    /// [`VERSION`] and any payload the walk does not consume exactly.
    pub fn parse(bytes: &[u8]) -> Result<Self, CampaignInfoError> {
        let mut c = Cursor::new(bytes);
        let version = c.u32("version")?;
        if version != VERSION {
            return Err(CampaignInfoError::UnsupportedVersion(version));
        }
        let header_words = [c.u32("header word")?, c.u32("header word")?];
        let strings = [c.string()?, c.string()?, c.string()?, c.string()?];
        let words_after_strings = [c.u32("word")?, c.u32("word")?];
        let string_after_words = c.string()?;
        let texture_path = c.string()?;
        let opaque = c.array::<OPAQUE_BLOCK_LEN>("opaque block")?;
        let floats = [c.f32("float")?, c.f32("float")?];
        let bytes_after_floats = c.array::<BYTE_QUAD_LEN>("byte block")?;
        let word_after_bytes = c.u32("word")?;

        let entry_count = c.count("entry", ENTRY_MIN_BYTES)?;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            entries.push(CampaignEntry {
                word: c.u32("entry word")?,
                strings: [c.string()?, c.string()?, c.string()?],
            });
        }

        let trailing_count = c.count("trailing entry", TRAILING_MIN_BYTES)?;
        let mut trailing_entries = Vec::with_capacity(trailing_count);
        for _ in 0..trailing_count {
            trailing_entries.push(TrailingEntry {
                strings: [c.string()?, c.string()?],
            });
        }

        c.finish()?;
        Ok(Self {
            version,
            header_words,
            strings,
            words_after_strings,
            string_after_words,
            texture_path,
            opaque,
            floats,
            bytes_after_floats,
            word_after_bytes,
            entries,
            trailing_entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Map;
    use std::path::Path;

    const TEXTURE: &str = r"UI\Widgets\Glues\Minimap-CustomCampaign-Orc";

    /// Appends the fields of one file in the measured order.
    #[derive(Default)]
    struct Fixture(Vec<u8>);

    impl Fixture {
        fn u32(mut self, v: u32) -> Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn f32(self, v: f32) -> Self {
            self.u32(v.to_bits())
        }

        fn bytes(mut self, b: &[u8]) -> Self {
            self.0.extend_from_slice(b);
            self
        }

        fn str(mut self, s: &str) -> Self {
            self.0.extend_from_slice(s.as_bytes());
            self.0.push(0);
            self
        }

        fn into_bytes(self) -> Vec<u8> {
            self.0
        }
    }

    /// Everything before the first list count, in the measured layout.
    fn header() -> Fixture {
        Fixture::default()
            .u32(VERSION)
            .u32(158)
            .u32(6030)
            .str("TRIGSTR_001")
            .str("TRIGSTR_002")
            .str("TRIGSTR_004")
            .str("TRIGSTR_003")
            .u32(1)
            .u32(0)
            .str("")
            .str(TEXTURE)
            .bytes(&[0; OPAQUE_BLOCK_LEN])
            .f32(2000.0)
            .f32(0.5)
            .bytes(&[0x4d, 0x33, 0x00, 0xff])
            .u32(1)
    }

    fn count(n: usize) -> u32 {
        u32::try_from(n).unwrap()
    }

    fn file(entries: &[(u32, [&str; 3])], trailing: &[[&str; 2]]) -> Vec<u8> {
        let mut f = header().u32(count(entries.len()));
        for (word, strings) in entries {
            f = f.u32(*word).str(strings[0]).str(strings[1]).str(strings[2]);
        }
        f = f.u32(count(trailing.len()));
        for pair in trailing {
            f = f.str(pair[0]).str(pair[1]);
        }
        f.into_bytes()
    }

    #[test]
    fn fields_land_at_their_measured_positions() {
        let bytes = file(
            &[(7, ["TRIGSTR_035", "TRIGSTR_010", "a.w3m"])],
            &[["", "b.w3m"]],
        );
        let info = CampaignInfo::parse(&bytes).unwrap();
        assert_eq!(info.version, 1);
        assert_eq!(info.header_words, [158, 6030]);
        assert_eq!(
            info.strings,
            ["TRIGSTR_001", "TRIGSTR_002", "TRIGSTR_004", "TRIGSTR_003"]
        );
        assert_eq!(info.words_after_strings, [1, 0]);
        assert_eq!(info.string_after_words, "");
        assert_eq!(info.texture_path, TEXTURE);
        assert_eq!(info.opaque, [0; OPAQUE_BLOCK_LEN]);
        assert_eq!(info.floats, [2000.0, 0.5]);
        assert_eq!(info.bytes_after_floats, [0x4d, 0x33, 0x00, 0xff]);
        assert_eq!(info.word_after_bytes, 1);
        assert_eq!(
            info.entries,
            [CampaignEntry {
                word: 7,
                strings: [
                    "TRIGSTR_035".to_string(),
                    "TRIGSTR_010".to_string(),
                    "a.w3m".to_string()
                ],
            }]
        );
        assert_eq!(
            info.trailing_entries,
            [TrailingEntry {
                strings: [String::new(), "b.w3m".to_string()],
            }]
        );
    }

    #[test]
    fn zero_counts_parse_to_empty_lists() {
        let info = CampaignInfo::parse(&file(&[], &[])).unwrap();
        assert!(info.entries.is_empty());
        assert!(info.trailing_entries.is_empty());
    }

    #[test]
    fn empty_input_is_refused_at_the_version_word() {
        assert_eq!(
            CampaignInfo::parse(&[]),
            Err(CampaignInfoError::Truncated {
                what: "version",
                at: 0,
                have: 0,
                need: WORD_BYTES,
            })
        );
    }

    #[test]
    fn every_truncation_of_a_valid_file_is_refused() {
        let bytes = file(
            &[(1, ["a", "b", "c.w3m"]), (2, ["", "", "d.w3m"])],
            &[["", "e.w3m"]],
        );
        for n in 0..bytes.len() {
            assert!(
                CampaignInfo::parse(&bytes[..n]).is_err(),
                "prefix of {n} bytes parsed"
            );
        }
    }

    #[test]
    fn unsupported_version_is_refused() {
        let mut bytes = file(&[], &[]);
        bytes[..WORD_BYTES].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            CampaignInfo::parse(&bytes),
            Err(CampaignInfoError::UnsupportedVersion(2))
        );
    }

    #[test]
    fn trailing_bytes_are_refused() {
        let mut bytes = file(&[], &[]);
        let end = bytes.len();
        bytes.push(0);
        assert_eq!(
            CampaignInfo::parse(&bytes),
            Err(CampaignInfoError::TrailingBytes { at: end, have: 1 })
        );
    }

    #[test]
    fn unterminated_string_is_refused_at_its_offset() {
        let bytes = Fixture::default()
            .u32(VERSION)
            .u32(0)
            .u32(0)
            .bytes(b"abc")
            .into_bytes();
        assert_eq!(
            CampaignInfo::parse(&bytes),
            Err(CampaignInfoError::UnterminatedString { at: 12 })
        );
    }

    #[test]
    fn non_utf8_string_is_refused_not_replaced() {
        let bytes = Fixture::default()
            .u32(VERSION)
            .u32(0)
            .u32(0)
            .bytes(&[0xff, 0])
            .into_bytes();
        assert_eq!(
            CampaignInfo::parse(&bytes),
            Err(CampaignInfoError::InvalidUtf8 { at: 12 })
        );
    }

    #[test]
    fn entry_count_beyond_the_bytes_is_refused_before_allocation() {
        let bytes = header().u32(u32::MAX).into_bytes();
        assert_eq!(
            CampaignInfo::parse(&bytes),
            Err(CampaignInfoError::CountExceedsBytes {
                what: "entry",
                at: bytes.len(),
                have: 0,
            })
        );
    }

    #[test]
    fn entry_count_is_accepted_exactly_at_the_byte_limit() {
        // Four minimal entries fill 4 * ENTRY_MIN_BYTES. The trailing count
        // word adds WORD_BYTES more, which is not enough for a fifth entry.
        let minimal = [(0, ["", "", ""]); 4];
        let fits = CampaignInfo::parse(&file(&minimal, &[])).unwrap();
        assert_eq!(fits.entries.len(), 4);

        let mut too_many = header().u32(5);
        for _ in 0..4 {
            too_many = too_many.u32(0).str("").str("").str("");
        }
        let too_many = too_many.u32(0).into_bytes();
        assert!(matches!(
            CampaignInfo::parse(&too_many),
            Err(CampaignInfoError::CountExceedsBytes { what: "entry", .. })
        ));
    }

    #[test]
    fn single_byte_mutations_never_panic() {
        let bytes = file(
            &[(9, ["TRIGSTR_035", "x", "a.w3m"]), (3, ["", "", "b.w3m"])],
            &[["", "c.w3m"]],
        );
        for at in 0..bytes.len() {
            for value in 0..=u8::MAX {
                let mut mutated = bytes.clone();
                mutated[at] = value;
                let _ = CampaignInfo::parse(&mutated);
            }
        }
    }

    /// Reads `war3campaign.w3f` from `DemoCampaign.w3n`, the one `.w3n` in the
    /// 1.28c install. `PEACECRAFT3_INSTALL` must name that install's
    /// `wc3/game/app` directory; the test reads the `War3x.mpq` member and never
    /// writes.
    #[test]
    #[ignore = "reads real assets: set PEACECRAFT3_INSTALL to the 1.28c wc3/game/app directory"]
    fn demo_campaign_info_matches_the_shipped_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let dir =
            std::env::var_os("PEACECRAFT3_INSTALL").ok_or("PEACECRAFT3_INSTALL is not set")?;
        let mut set = pcx_mpq::MpqSet::open_wc3(Path::new(&dir))?;
        let container = set.extract("DemoCampaign.w3n")?;
        let w3f = Map::parse(&container)?.extract(CAMPAIGN_INFO_NAME)?;
        assert_eq!(w3f.len(), 303);

        let info = CampaignInfo::parse(&w3f)?;
        assert_eq!(info.header_words, [158, 6030]);
        assert_eq!(
            info.strings,
            ["TRIGSTR_001", "TRIGSTR_002", "TRIGSTR_004", "TRIGSTR_003"]
        );
        assert_eq!(info.texture_path, TEXTURE);
        assert_eq!(info.floats, [2000.0, 0.5]);
        assert_eq!(info.bytes_after_floats, [0x4d, 0x33, 0x00, 0xff]);
        let entry_words: Vec<u32> = info.entries.iter().map(|e| e.word).collect();
        assert_eq!(entry_words, [0x0012_f8e5, 0x0012_f8e4, 0x0012_f8e4]);
        let map_paths: Vec<&str> = info.entries.iter().map(|e| e.strings[2].as_str()).collect();
        assert_eq!(map_paths, ["Demo03.w3m", "Demo04.w3m", "Demo05.w3m"]);
        let trailing_paths: Vec<&str> = info
            .trailing_entries
            .iter()
            .map(|e| e.strings[1].as_str())
            .collect();
        assert_eq!(trailing_paths, ["Demo03.w3m", "Demo04.w3m", "Demo05.w3m"]);
        Ok(())
    }
}
