//! Explicit selected-archive gate; originals stay in memory and are never saved.
use std::io::Read;

use pcx_mpq::checked::{CheckedArchive, ReadBudget};
use pcx_objects::editor::campaign_index::{CampaignSlot, IndexLimits, parse_campaign_index};

#[test]
#[ignore = "requires caller-selected PEACECRAFT3_CAMPAIGN_ARCHIVE containing both CampaignStrings members"]
fn selected_original_campaign_indices() {
    let path = std::env::var_os("PEACECRAFT3_CAMPAIGN_ARCHIVE")
        .expect("set PEACECRAFT3_CAMPAIGN_ARCHIVE; missing originals are not a successful skip");
    let file = std::fs::File::open(&path).expect("open selected archive");
    let size = usize::try_from(file.metadata().expect("archive metadata").len())
        .expect("archive length fits usize");
    // Resource policies for this test, not native archive or campaign limits.
    let max_archive = 1024 * 1024 * 1024;
    let max_member = 1024 * 1024;
    assert!(size <= max_archive, "selected archive exceeds test budget");
    let capacity = size.checked_add(1).expect("archive read sentinel fits");
    std::alloc::Layout::array::<u8>(capacity).expect("archive layout fits");
    let mut raw = Vec::new();
    raw.try_reserve_exact(capacity).expect("archive allocation");
    file.take(capacity as u64)
        .read_to_end(&mut raw)
        .expect("read selected archive");
    assert_eq!(raw.len(), size, "selected archive changed while reading");
    let source = std::path::Path::new(&path).display().to_string();
    let archive = CheckedArchive::parse(
        &raw,
        &source,
        ReadBudget {
            max_archive_bytes: max_archive,
            max_archive_table_bytes: 64 * 1024 * 1024,
            max_raw_packed_bytes: max_member,
            max_decoded_bytes: max_member,
        },
    )
    .expect("checked selected archive");
    for member in [r"UI\CampaignStrings.txt", r"UI\CampaignStrings_exp.txt"] {
        let bytes = archive
            .extract(member, max_member)
            .expect("extract selected member")
            .expect("selected member must exist");
        let label = format!("{source}::{member}");
        let index = parse_campaign_index(
            &label,
            &bytes,
            IndexLimits {
                max_bytes: max_member,
                max_campaign_slots: 32,
                max_missions_per_campaign: 256,
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(index.member, label);
        assert!(index.first_index_entry().is_some());
        let missions: usize = index
            .slots
            .iter()
            .map(|slot| match slot {
                CampaignSlot::Blank => 0,
                CampaignSlot::Entry(entry) => entry.missions.len(),
            })
            .sum();
        println!(
            "source={label} bytes={} fnv1a64={:016x} slots={} missions={missions}",
            bytes.len(),
            pcx_mpq::fnv1a64(&bytes),
            index.slots.len()
        );
    }
}
