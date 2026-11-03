//! Visual pacing modes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PacingMode {
    #[default]
    WordHighlight,
    MovingUnderline,
    ReadingPointer,
    PhrasePacing,
    Rsvp,
}

impl PacingMode {
    pub fn all() -> &'static [PacingMode] {
        &[
            PacingMode::WordHighlight,
            PacingMode::MovingUnderline,
            PacingMode::ReadingPointer,
            PacingMode::PhrasePacing,
            PacingMode::Rsvp,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            PacingMode::WordHighlight => "Word Highlight",
            PacingMode::MovingUnderline => "Moving Underline",
            PacingMode::ReadingPointer => "Reading Pointer",
            PacingMode::PhrasePacing => "Phrase Pacing",
            PacingMode::Rsvp => "RSVP",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().iter().copied().find(|m| m.label() == label)
    }
}

/// Compute how many words to advance for a given mode and phrase size.
pub fn advance_step(mode: PacingMode, phrase_size: usize) -> usize {
    match mode {
        PacingMode::PhrasePacing => phrase_size.max(1),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_advance_respects_size() {
        assert_eq!(advance_step(PacingMode::PhrasePacing, 4), 4);
        assert_eq!(advance_step(PacingMode::WordHighlight, 4), 1);
    }
}
