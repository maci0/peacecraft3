//! Cell storage: entries sorted by `(row, column)` over one shared text buffer.
use std::collections::BTreeMap;

/// A cell's coordinate and its byte range in [`Cells::text`].
#[derive(Debug, Clone, Copy)]
struct Entry {
    key: (u32, u32),
    start: usize,
    end: usize,
}

/// Row offsets are kept while the highest row is at most this many rows per cell, plus slack.
const ROWS_PER_CELL: usize = 4;
const ROW_SLACK: usize = 64;

/// The populated cells of a table, in `(row, column)` order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cells {
    entries: Vec<Entry>,
    text: String,
    /// `row_starts[r]` is the position of the first entry in row `r` or later, for
    /// every row up to one past the highest. Empty when the rows are too sparse
    /// for a table of that size; lookups then search the entries.
    row_starts: Vec<u32>,
}

impl Cells {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    fn value(&self, entry: &Entry) -> &str {
        &self.text[entry.start..entry.end]
    }

    /// The entries of `row`, in column order.
    fn row_entries(&self, row: u32) -> &[Entry] {
        let at = row as usize;
        if self.row_starts.is_empty() {
            let first = self.entries.partition_point(|e| e.key.0 < row);
            let end = first + self.entries[first..].partition_point(|e| e.key.0 == row);
            return &self.entries[first..end];
        }
        match (
            self.row_starts.get(at),
            at.checked_add(1).and_then(|next| self.row_starts.get(next)),
        ) {
            (Some(&first), Some(&end)) => &self.entries[first as usize..end as usize],
            _ => &[],
        }
    }

    pub(crate) fn get(&self, key: (u32, u32)) -> Option<&str> {
        let entries = self.row_entries(key.0);
        let at = entries.binary_search_by_key(&key.1, |e| e.key.1).ok()?;
        Some(self.value(&entries[at]))
    }

    pub(crate) fn contains(&self, key: (u32, u32)) -> bool {
        self.get(key).is_some()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&(u32, u32), &str)> {
        self.entries.iter().map(|e| (&e.key, self.value(e)))
    }

    /// The cells of `row`, in column order.
    pub(crate) fn row(&self, row: u32) -> impl Iterator<Item = (u32, &str)> {
        self.row_entries(row)
            .iter()
            .map(|e| (e.key.1, self.value(e)))
    }

    /// Builds the row offsets when the rows are dense enough to afford them.
    fn index_rows(&mut self) {
        let Some(last) = self.entries.last().map(|e| e.key.0 as usize) else {
            return;
        };
        if self.entries.len() >= u32::MAX as usize
            || last > self.entries.len() * ROWS_PER_CELL + ROW_SLACK
        {
            return;
        }
        let mut starts = Vec::with_capacity(last + 2);
        for (at, entry) in self.entries.iter().enumerate() {
            // Entries are sorted, so each new row begins at `at` for every row up to it.
            starts.resize(entry.key.0 as usize + 1, at as u32);
        }
        starts.push(self.entries.len() as u32);
        self.row_starts = starts;
    }
}

/// Collects cells while parsing. Cells arriving in strictly ascending coordinate
/// order, which every shipped table does, are appended directly. The first
/// out-of-order or repeated coordinate adds a coordinate index; later cells are
/// checked against it, and [`CellBuilder::finish`] sorts the entries once.
#[derive(Default)]
pub(crate) struct CellBuilder {
    cells: Cells,
    /// Coordinate to position in `cells.entries`, once arrival order stops being sorted.
    index: Option<BTreeMap<(u32, u32), usize>>,
}

impl CellBuilder {
    /// Stores `value` unless the coordinate is already present; false when it was.
    pub(crate) fn store(&mut self, key: (u32, u32), value: &str) -> bool {
        let entries = &self.cells.entries;
        if self.index.is_none() && entries.last().is_none_or(|last| last.key < key) {
            self.cells.push(key, value);
            return true;
        }
        let index = self.index.get_or_insert_with(|| {
            entries
                .iter()
                .enumerate()
                .map(|(at, e)| (e.key, at))
                .collect()
        });
        if index.contains_key(&key) {
            return false;
        }
        index.insert(key, entries.len());
        self.cells.push(key, value);
        true
    }

    /// The cells, sorted by coordinate.
    pub(crate) fn finish(mut self) -> Cells {
        if let Some(index) = self.index {
            let entries = &self.cells.entries;
            self.cells.entries = index.into_values().map(|at| entries[at]).collect();
        }
        self.cells.index_rows();
        self.cells
    }
}

impl Cells {
    fn push(&mut self, key: (u32, u32), value: &str) {
        let start = self.text.len();
        self.text.push_str(value);
        self.entries.push(Entry {
            key,
            start,
            end: self.text.len(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built(cells: &[((u32, u32), &str)]) -> (Cells, Vec<bool>) {
        let mut builder = CellBuilder::default();
        let stored = cells.iter().map(|&(k, v)| builder.store(k, v)).collect();
        (builder.finish(), stored)
    }

    fn listed(cells: &Cells) -> Vec<((u32, u32), &str)> {
        cells.iter().map(|(&k, v)| (k, v)).collect()
    }

    #[test]
    fn ascending_cells_are_kept_in_arrival_order() {
        let (cells, stored) = built(&[((1, 1), "a"), ((1, 3), "b"), ((2, 1), "")]);
        assert_eq!(stored, [true; 3]);
        assert_eq!(listed(&cells), [((1, 1), "a"), ((1, 3), "b"), ((2, 1), "")]);
        assert_eq!(cells.get((1, 3)), Some("b"));
        assert_eq!(cells.get((2, 1)), Some(""));
        assert_eq!(cells.get((1, 2)), None);
        assert_eq!(cells.len(), 3);
    }

    #[test]
    fn out_of_order_cells_are_sorted_and_the_first_repeat_wins() {
        let (cells, stored) = built(&[
            ((3, 1), "late"),
            ((1, 2), "second"),
            ((1, 1), "first"),
            ((3, 1), "repeat"),
            ((1, 2), "again"),
            ((2, 9), "mid"),
        ]);
        assert_eq!(stored, [true, true, true, false, false, true]);
        assert_eq!(
            listed(&cells),
            [
                ((1, 1), "first"),
                ((1, 2), "second"),
                ((2, 9), "mid"),
                ((3, 1), "late")
            ]
        );
        assert!(cells.contains((2, 9)));
        assert!(!cells.contains((2, 8)));
    }

    #[test]
    fn a_repeat_of_the_last_ascending_cell_is_refused() {
        let (cells, stored) = built(&[((1, 1), "a"), ((1, 1), "b")]);
        assert_eq!(stored, [true, false]);
        assert_eq!(listed(&cells), [((1, 1), "a")]);
    }

    #[test]
    fn row_returns_only_that_rows_cells_in_column_order() {
        let (cells, _) = built(&[((2, 5), "x"), ((1, 4), "y"), ((2, 1), "z"), ((4, 1), "w")]);
        assert_eq!(cells.row(2).collect::<Vec<_>>(), [(1, "z"), (5, "x")]);
        assert_eq!(cells.row(1).collect::<Vec<_>>(), [(4, "y")]);
        assert_eq!(cells.row(3).count(), 0);
        assert_eq!(cells.row(u32::MAX).count(), 0);
        assert_eq!(Cells::default().row(1).count(), 0);
    }

    #[test]
    fn rows_far_beyond_the_cell_count_are_searched_without_a_row_table() {
        let (cells, _) = built(&[((1, 1), "a"), ((1, 2), "b"), ((4_000_000_000, 7), "far")]);
        assert!(cells.row_starts.is_empty());
        assert_eq!(cells.get((4_000_000_000, 7)), Some("far"));
        assert_eq!(cells.get((4_000_000_000, 6)), None);
        assert_eq!(cells.get((2, 1)), None);
        assert_eq!(cells.row(1).collect::<Vec<_>>(), [(1, "a"), (2, "b")]);
        assert_eq!(cells.row(u32::MAX).count(), 0);
    }

    #[test]
    fn dense_rows_use_the_row_table_and_agree_with_a_search() {
        let (cells, _) = built(&[((3, 2), "x"), ((1, 1), "y"), ((3, 1), "z"), ((6, 5), "w")]);
        assert_eq!(cells.row_starts, [0, 0, 1, 1, 3, 3, 3, 4]);
        for row in 0..9 {
            for column in 0..8 {
                let wanted = cells
                    .iter()
                    .find(|(key, _)| **key == (row, column))
                    .map(|(_, v)| v);
                assert_eq!(cells.get((row, column)), wanted, "({row}, {column})");
            }
        }
        assert_eq!(cells.row(3).collect::<Vec<_>>(), [(1, "z"), (2, "x")]);
        assert_eq!(cells.row(7).count(), 0);
    }
}
