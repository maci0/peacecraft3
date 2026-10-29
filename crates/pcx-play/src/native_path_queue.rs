//! The priority queue used by the selected-client reach search.
//!
//! The live placement query is not connected to this queue.

/// A queue entry carries the generation used to reject stale paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub priority: u32,
    pub node: u32,
    pub generation: u32,
}

/// A minimum heap with the selected client's equal-priority ordering.
#[derive(Default)]
pub struct Queue(Vec<Entry>);

impl Queue {
    /// Insert one entry. Equal keys move above their parent.
    pub fn push(&mut self, entry: Entry) -> Result<(), std::collections::TryReserveError> {
        self.0.try_reserve(1)?;
        let mut index = self.0.len();
        self.0.push(entry);
        while index > 0 {
            let parent = (index - 1) / 2;
            if self.0[parent].priority < entry.priority {
                break;
            }
            self.0[index] = self.0[parent];
            index = parent;
        }
        self.0[index] = entry;
        Ok(())
    }

    /// Remove one entry, charging the work budget before checking its generation.
    pub fn pop(&mut self, remaining: &mut usize) -> Option<Entry> {
        if *remaining == 0 || self.0.is_empty() {
            return None;
        }
        *remaining -= 1;
        let last = self.0.pop()?;
        if self.0.is_empty() {
            return Some(last);
        }
        let result = self.0[0];
        let mut index = 0;
        loop {
            let left = index * 2 + 1;
            if left >= self.0.len() {
                break;
            }
            let right = left + 1;
            let child = if right < self.0.len() && self.0[right].priority <= self.0[left].priority {
                right
            } else {
                left
            };
            if last.priority <= self.0[child].priority {
                break;
            }
            self.0[index] = self.0[child];
            index = child;
        }
        self.0[index] = last;
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_keys_promote_on_insert_and_use_right_child_on_removal() {
        let mut queue = Queue::default();
        for node in 1..=5 {
            queue
                .push(Entry {
                    priority: 7,
                    node,
                    generation: 0,
                })
                .unwrap();
        }
        let mut remaining = 5;
        let order: Vec<_> = std::iter::from_fn(|| queue.pop(&mut remaining))
            .map(|entry| entry.node)
            .collect();
        assert_eq!(order, [5, 3, 1, 2, 4]);
        assert_eq!(remaining, 0);
        for (priority, node) in [(0, 1), (1, 2), (1, 3), (2, 4)] {
            queue
                .push(Entry {
                    priority,
                    node,
                    generation: 0,
                })
                .unwrap();
        }
        remaining = 2;
        assert_eq!(queue.pop(&mut remaining).unwrap().node, 1);
        assert_eq!(queue.pop(&mut remaining).unwrap().node, 3);
    }

    #[test]
    fn stale_removal_consumes_budget() {
        let mut queue = Queue::default();
        queue
            .push(Entry {
                priority: 1,
                node: 4,
                generation: 0,
            })
            .unwrap();
        queue
            .push(Entry {
                priority: 2,
                node: 4,
                generation: 1,
            })
            .unwrap();
        let mut remaining = 1;
        assert_ne!(queue.pop(&mut remaining).unwrap().generation, 1);
        assert_eq!(queue.pop(&mut remaining), None);
        remaining = 1;
        assert_eq!(queue.pop(&mut remaining).unwrap().generation, 1);
    }
}
