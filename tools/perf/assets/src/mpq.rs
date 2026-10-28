//! MPQ open, listfile and member extraction (unchecked, bounded and checked readers).
use pcx_mpq::{ExtractionBudget, MpqArchive, checked::CheckedArchive, checked::ReadBudget};

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, AUDIO_MEMBERS, CHECKED_MAPS, CHECKED_MEMBERS, EXTRACT_MEMBERS},
};

pub const WORKLOADS: [Workload; 7] = [
    Workload {
        name: "mpq-open-v1",
        summary: "open the four campaign archives and decode hash and block tables",
        prepare: open,
    },
    Workload {
        name: "mpq-codec-table-v1",
        summary: "read the Huffman and ADPCM tables from the game executable",
        prepare: codec_table,
    },
    Workload {
        name: "mpq-extract-v1",
        summary: "unchecked MpqSet::extract over a fixed member list",
        prepare: extract,
    },
    Workload {
        name: "mpq-extract-bounded-v1",
        summary: "extract_bounded over the same member list",
        prepare: extract_bounded,
    },
    Workload {
        name: "mpq-audio-v1",
        summary: "extract_bounded over 24 Huffman and ADPCM speech files",
        prepare: audio,
    },
    Workload {
        name: "mpq-listfile-v1",
        summary: "union (listfile) of the four archives",
        prepare: listfile,
    },
    Workload {
        name: "mpq-checked-v1",
        summary: "CheckedArchive parse and member extraction over fixed maps",
        prepare: checked_maps,
    },
];

fn fold_archive(out: &mut Fnv, archive: &MpqArchive) {
    for word in archive.header.as_words() {
        out.u32(word);
    }
    for e in archive.hash_entries() {
        out.u32(e.name1);
        out.u32(e.name2);
        out.u32(u32::from(e.locale) | u32::from(e.platform) << 16);
        out.u32(e.block_index);
    }
    for b in archive.block_entries() {
        out.u32(b.file_pos);
        out.u32(b.compressed_size);
        out.u32(b.file_size);
        out.u32(b.flags);
    }
}

fn open(ctx: &Context) -> Result<Prepared, Failure> {
    let paths = input::archive_paths(&ctx.install)?;
    let mut input_digest = Fnv::default();
    for path in &paths {
        let len = std::fs::metadata(path)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .len();
        input_digest.text(&path.file_name().unwrap_or_default().to_string_lossy());
        input_digest.u64(len);
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut opened = Vec::new();
            for path in &paths {
                opened.push(input::open_archive(path)?);
            }
            let entries: usize = opened
                .iter()
                .map(|a| a.hash_entries().len() + a.block_entries().len())
                .sum();
            Ok(Outcome {
                work: vec![
                    ("archives", opened.len() as u64),
                    ("table_entries", entries as u64),
                ],
                fold: Box::new(move |out| opened.iter().for_each(|a| fold_archive(out, a))),
            })
        }),
    })
}

fn codec_table(ctx: &Context) -> Result<Prepared, Failure> {
    let path = ctx.install.join(input::CODEC_SOURCE);
    let image = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut input_digest = Fnv::default();
    input_digest.bytes(&image);
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let table = pcx_mpq::huffman::HuffmanTable::from_pe(&image, &path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(Outcome {
                work: vec![("image_bytes", image.len() as u64)],
                fold: Box::new(move |out| {
                    out.u64(table.table_va.into());
                    out.u64(table.row_stride as u64);
                    out.u64(table.file_backed_bytes as u64);
                    out.u64(table.zero_filled_bytes as u64);
                    if let Some(adpcm) = &table.adpcm {
                        out.u32(adpcm.steps_va);
                        out.u32(adpcm.index_va);
                        adpcm.steps.iter().for_each(|s| out.u32(*s));
                        adpcm.index_adjust.iter().for_each(|s| out.u32(*s as u32));
                    }
                }),
            })
        }),
    })
}

fn total(chunks: &[Vec<u8>]) -> u64 {
    chunks.iter().map(|c| c.len() as u64).sum()
}

fn fold_members(out: &mut Fnv, chunks: &[Vec<u8>]) {
    for (path, bytes) in EXTRACT_MEMBERS.iter().zip(chunks) {
        out.text(path);
        out.bytes(bytes);
    }
}

fn input_identity(set: &pcx_mpq::MpqSet) -> Fnv {
    let mut input_digest = Fnv::default();
    for archive in set.members() {
        input_digest.u64(archive.byte_len());
    }
    for path in EXTRACT_MEMBERS {
        input_digest.text(path);
    }
    input_digest
}

fn extract(ctx: &Context) -> Result<Prepared, Failure> {
    let mut set = input::open_set_with_codec(&ctx.install)?;
    Ok(Prepared {
        input: input_identity(&set),
        run: Box::new(move || {
            let mut chunks = Vec::new();
            for path in EXTRACT_MEMBERS {
                chunks.push(set.extract(path).map_err(|e| format!("{path}: {e}"))?);
            }
            Ok(Outcome {
                work: vec![
                    ("members", chunks.len() as u64),
                    ("decoded_bytes", total(&chunks)),
                ],
                fold: Box::new(move |out| fold_members(out, &chunks)),
            })
        }),
    })
}

const BOUNDED_BYTES: usize = 256 << 20;

fn extract_bounded(ctx: &Context) -> Result<Prepared, Failure> {
    let table = input::codec_table(&ctx.install)?;
    let mut set = input::open_set(&ctx.install)?;
    let mut homes = Vec::new();
    for path in EXTRACT_MEMBERS {
        let (index, _) = set
            .locate(path)
            .ok_or_else(|| format!("{path}: not in the four archives"))?;
        homes.push(index);
    }
    Ok(Prepared {
        input: input_identity(&set),
        run: Box::new(move || {
            let budget = ExtractionBudget {
                max_raw_packed_bytes: BOUNDED_BYTES,
                max_decoded_bytes: BOUNDED_BYTES,
            };
            let mut chunks = Vec::new();
            for (path, home) in EXTRACT_MEMBERS.iter().zip(&homes) {
                let bytes = set.members_mut()[*home]
                    .extract_bounded(path, budget, Some(&table))
                    .map_err(|e| e.to_string())?;
                chunks.push(bytes);
            }
            Ok(Outcome {
                work: vec![
                    ("members", chunks.len() as u64),
                    ("decoded_bytes", total(&chunks)),
                ],
                fold: Box::new(move |out| fold_members(out, &chunks)),
            })
        }),
    })
}

fn listfile(ctx: &Context) -> Result<Prepared, Failure> {
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let mut input_digest = Fnv::default();
    for archive in set.members() {
        input_digest.u64(archive.byte_len());
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let listed = set.file_list().map_err(|e| e.to_string())?;
            Ok(Outcome {
                work: vec![("paths", listed.len() as u64)],
                fold: Box::new(move |out| listed.iter().for_each(|p| out.text(p))),
            })
        }),
    })
}

const CHECKED_BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 256 << 20,
    max_archive_table_bytes: 64 << 20,
    max_raw_packed_bytes: 64 << 20,
    max_decoded_bytes: 64 << 20,
};

fn checked_maps(ctx: &Context) -> Result<Prepared, Failure> {
    let maps = input::read_maps(&ctx.install, &CHECKED_MAPS)?;
    let mut input_digest = Fnv::default();
    for (name, bytes) in &maps {
        input_digest.text(name);
        input_digest.bytes(bytes);
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut found: Vec<Option<Vec<u8>>> = Vec::new();
            for (name, bytes) in &maps {
                let map = pcx_map::Map::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
                let archive = CheckedArchive::parse(map.archive(), name, CHECKED_BUDGET)
                    .map_err(|e| e.to_string())?;
                for member in CHECKED_MEMBERS {
                    found.push(
                        archive
                            .extract(member, CHECKED_BUDGET.max_decoded_bytes)
                            .map_err(|e| e.to_string())?,
                    );
                }
            }
            let bytes: u64 = found.iter().flatten().map(|b| b.len() as u64).sum();
            Ok(Outcome {
                work: vec![
                    ("members_read", found.len() as u64),
                    ("decoded_bytes", bytes),
                ],
                fold: Box::new(move |out| {
                    for member in &found {
                        match member {
                            Some(bytes) => {
                                out.u32(1);
                                out.bytes(bytes);
                            }
                            None => out.u32(0),
                        }
                    }
                }),
            })
        }),
    })
}

fn audio(ctx: &Context) -> Result<Prepared, Failure> {
    let table = input::codec_table(&ctx.install)?;
    let mut set = input::open_set(&ctx.install)?;
    let mut input_digest = Fnv::default();
    let mut homes = Vec::new();
    for path in AUDIO_MEMBERS {
        let (index, block) = set
            .locate(path)
            .ok_or_else(|| format!("{path}: not in the four archives"))?;
        input_digest.text(path);
        input_digest.u32(block.file_size);
        homes.push(index);
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let budget = ExtractionBudget {
                max_raw_packed_bytes: BOUNDED_BYTES,
                max_decoded_bytes: BOUNDED_BYTES,
            };
            let mut chunks = Vec::new();
            for (path, home) in AUDIO_MEMBERS.iter().zip(&homes) {
                let bytes = set.members_mut()[*home]
                    .extract_bounded(path, budget, Some(&table))
                    .map_err(|e| e.to_string())?;
                chunks.push(bytes);
            }
            Ok(Outcome {
                work: vec![
                    ("members", chunks.len() as u64),
                    ("decoded_bytes", total(&chunks)),
                ],
                fold: Box::new(move |out| chunks.iter().for_each(|c| out.bytes(c))),
            })
        }),
    })
}
