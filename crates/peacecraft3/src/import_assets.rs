//! Checked staging of canonical names and renderer aliases before publication.
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct HydratedAssets {
    entries: BTreeMap<String, Vec<u8>>,
    stored_bytes: usize,
    limit: usize,
}

impl HydratedAssets {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            stored_bytes: 0,
            limit,
        }
    }

    /// Names are normalized by the caller using the same rule as its reader.
    /// Equal content may share a name. Unequal content rejects the whole insert
    /// before any new canonical name or alias becomes visible in this staging map.
    pub(crate) fn insert(
        &mut self,
        source: &str,
        canonical: String,
        alias: Option<String>,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let names = [Some(canonical), alias]
            .into_iter()
            .flatten()
            .collect::<BTreeSet<_>>();
        let mut added = 0usize;
        for name in &names {
            match self.entries.get(name) {
                Some(previous) if previous != &bytes => {
                    return Err(format!(
                        "AMBIGUOUS_ASSET_ALIAS: {name:?} conflicts with hydrated content while registering {source:?}"
                    ));
                }
                Some(_) => {}
                None => {
                    added = added
                        .checked_add(bytes.len())
                        .ok_or("hydration size overflow")?
                }
            }
        }
        let total = self
            .stored_bytes
            .checked_add(added)
            .ok_or("hydration size overflow")?;
        if total > self.limit {
            return Err(format!(
                "runtime hydration exceeds {} byte budget",
                self.limit
            ));
        }
        for name in names {
            self.entries.entry(name).or_insert_with(|| bytes.clone());
        }
        self.stored_bytes = total;
        Ok(())
    }

    pub(crate) fn finish(self) -> BTreeMap<String, Vec<u8>> {
        self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unequal_aliases_reject_without_adding_the_new_canonical_name() {
        let mut assets = HydratedAssets::new(32);
        assets
            .insert(
                "first",
                "first/font.ttf".into(),
                Some("menu/fonts/font.ttf".into()),
                vec![1],
            )
            .unwrap();
        let before = assets.entries.clone();
        let error = assets
            .insert(
                "second",
                "second/font.ttf".into(),
                Some("menu/fonts/font.ttf".into()),
                vec![2],
            )
            .unwrap_err();
        assert!(error.contains("AMBIGUOUS_ASSET_ALIAS") && error.contains("second"));
        assert_eq!(assets.finish(), before);
    }

    #[test]
    fn equal_aliases_share_content_and_only_new_names_consume_budget() {
        let mut assets = HydratedAssets::new(3);
        assets
            .insert("first", "first".into(), Some("shared".into()), vec![1])
            .unwrap();
        assets
            .insert("second", "second".into(), Some("shared".into()), vec![1])
            .unwrap();
        assert_eq!(assets.stored_bytes, 3);
        assert_eq!(assets.finish().len(), 3);
    }

    #[test]
    fn canonical_conflicts_and_budget_failures_leave_existing_entries_unchanged() {
        let mut assets = HydratedAssets::new(2);
        assets
            .insert("first", "canonical".into(), None, vec![1])
            .unwrap();
        let before = assets.entries.clone();
        assert!(
            assets
                .insert("other", "canonical".into(), Some("fresh".into()), vec![2])
                .is_err()
        );
        assert_eq!(assets.entries, before);
        assert!(
            assets
                .insert("large", "larger".into(), None, vec![3, 4])
                .is_err()
        );
        assert_eq!(assets.finish(), before);
    }

    #[test]
    fn a_name_used_as_both_canonical_and_alias_is_stored_once() {
        let mut assets = HydratedAssets::new(1);
        assets
            .insert("same", "same".into(), Some("same".into()), vec![1])
            .unwrap();
        assert_eq!(assets.finish().len(), 1);
    }
}
