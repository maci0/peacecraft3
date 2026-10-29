//! The text of the latest refusal, posted by input handlers and drawn by the
//! application as the error line above the console. The client keeps one line:
//! a new refusal replaces the old one and restarts its timers
//! (`docs/re-notes/input-chat-overlays.md`).
use bevy::prelude::*;

/// What is on the error line and when it was posted, in `Time<Real>` seconds.
#[derive(Resource, Default, Debug)]
pub struct ErrorLine {
    posted: Option<Posted>,
    revision: u64,
}

#[derive(Debug)]
struct Posted {
    text: String,
    at: f64,
}

impl ErrorLine {
    /// Replaces the line, even with the same text, and restarts its timers.
    pub fn post(&mut self, text: impl Into<String>, now: f64) {
        self.posted = Some(Posted {
            text: text.into(),
            at: now,
        });
        self.revision += 1;
    }

    /// The text and the time it was posted.
    pub fn current(&self) -> Option<(&str, f64)> {
        self.posted
            .as_ref()
            .map(|posted| (posted.text.as_str(), posted.at))
    }

    /// Removes the line, for a new session.
    pub fn clear(&mut self) {
        if self.posted.take().is_some() {
            self.revision += 1;
        }
    }

    /// Changes whenever a post or a clear changes the line.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_post_replaces_the_line_and_restarts_it() {
        let mut line = ErrorLine::default();
        assert_eq!(line.current(), None);
        line.post("Not enough gold.", 2.0);
        assert_eq!(line.current(), Some(("Not enough gold.", 2.0)));
        let first = line.revision();
        line.post("Not enough gold.", 5.0);
        assert_eq!(line.current(), Some(("Not enough gold.", 5.0)));
        assert!(line.revision() > first, "the same text posts again");
        line.post("Not enough lumber.", 6.0);
        assert_eq!(line.current(), Some(("Not enough lumber.", 6.0)));
    }

    #[test]
    fn clearing_an_empty_line_changes_nothing() {
        let mut line = ErrorLine::default();
        line.clear();
        assert_eq!(line.revision(), 0);
        line.post("x", 0.0);
        line.clear();
        assert_eq!(line.current(), None);
        assert_eq!(line.revision(), 2);
    }
}
