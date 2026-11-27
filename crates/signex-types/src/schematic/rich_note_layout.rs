//! The Frame and Size sections of a rich text note (#1114, mockup row 203).
//! Stored beside the notes, as the format is:
//! `docs/architecture/snxsch-note-format.md`.

use serde::{Deserialize, Serialize};

/// What is drawn around a rich note.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteFrame {
    /// The text alone.
    #[default]
    None,
    /// An outline in the note's ink.
    Border,
    /// An outline over a fill in the sheet's paper tone.
    Filled,
}

impl NoteFrame {
    /// Every frame, in the order the Frame row shows them.
    pub const ALL: [Self; 3] = [Self::None, Self::Border, Self::Filled];

    /// The word a file, an operand and an answer write.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Border => "border",
            Self::Filled => "filled",
        }
    }

    /// The frame `word` names; `None` for a word this build does not know.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|frame| frame.as_str() == word)
    }
}

/// How a rich note's box is sized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteSize {
    /// As wide and as tall as the text.
    #[default]
    FitText,
    /// A set width; the height grows with the text.
    FixedWidth,
    /// A set width and height; text that does not fit is clipped.
    FixedBox,
}

impl NoteSize {
    /// Every size, in the order the Size row shows them.
    pub const ALL: [Self; 3] = [Self::FitText, Self::FixedWidth, Self::FixedBox];

    /// The word a file, an operand and an answer write.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FitText => "fit_text",
            Self::FixedWidth => "fixed_width",
            Self::FixedBox => "fixed_box",
        }
    }

    /// The size `word` names; `None` for a word this build does not know.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|size| size.as_str() == word)
    }

    /// Whether the width is set by the author rather than by the text.
    #[must_use]
    pub const fn has_width(self) -> bool {
        matches!(self, Self::FixedWidth | Self::FixedBox)
    }
}

/// The width a Fixed width or Fixed box note takes when none was set yet.
pub const DEFAULT_NOTE_WIDTH_MM: f64 = 50.8;
/// The height a Fixed box note takes when none was set yet.
pub const DEFAULT_NOTE_HEIGHT_MM: f64 = 25.4;
/// The smallest width or height a note box keeps.
pub const MIN_NOTE_EDGE_MM: f64 = 2.54;

/// The Frame and Size sections of a rich note. The default (no frame, no
/// padding, sized to the text) is what a note without the sections draws.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RichNoteLayout {
    #[serde(default)]
    pub frame: NoteFrame,
    /// Outline width in mm; `None` is Auto, the sheet's line width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_mm: Option<f64>,
    /// Space between the frame and the text, in mm.
    #[serde(default)]
    pub padding_mm: f64,
    #[serde(default)]
    pub size: NoteSize,
    /// The box width for Fixed width and Fixed box; 0 when never set.
    #[serde(default)]
    pub width_mm: f64,
    /// The box height for Fixed box; 0 when never set.
    #[serde(default)]
    pub height_mm: f64,
}

impl RichNoteLayout {
    /// Whether nothing differs from the default, which no file writes.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The width the text is laid out to, inside the padding: `None` when it
    /// is sized to the text.
    #[must_use]
    pub fn text_width_mm(&self) -> Option<f64> {
        self.size
            .has_width()
            .then(|| (self.box_width_mm() - 2.0 * self.padding_mm).max(MIN_NOTE_EDGE_MM))
    }

    /// The box width a Fixed width or Fixed box note draws at.
    #[must_use]
    pub fn box_width_mm(&self) -> f64 {
        if self.width_mm > 0.0 {
            self.width_mm.max(MIN_NOTE_EDGE_MM)
        } else {
            DEFAULT_NOTE_WIDTH_MM
        }
    }

    /// The box height a Fixed box note draws at; `None` for the other sizes.
    #[must_use]
    pub fn box_height_mm(&self) -> Option<f64> {
        (self.size == NoteSize::FixedBox).then(|| {
            if self.height_mm > 0.0 {
                self.height_mm.max(MIN_NOTE_EDGE_MM)
            } else {
                DEFAULT_NOTE_HEIGHT_MM
            }
        })
    }

    /// The box of a note whose text drew `text` (width, height) in mm: the
    /// text plus the padding, at the set width and height where the size
    /// says so.
    #[must_use]
    pub fn box_size_mm(&self, text: (f64, f64)) -> (f64, f64) {
        let pad = 2.0 * self.padding_mm.max(0.0);
        let width = if self.size.has_width() {
            self.box_width_mm()
        } else {
            text.0 + pad
        };
        let height = self.box_height_mm().unwrap_or(text.1 + pad);
        (width, height)
    }

    /// Whether `text` (width, height) in mm overflows a Fixed box note,
    /// whose overflow is clipped and marked.
    #[must_use]
    pub fn clips(&self, text: (f64, f64)) -> bool {
        self.box_height_mm()
            .is_some_and(|height| text.1 + 2.0 * self.padding_mm.max(0.0) > height + 1e-6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_frame_and_size_reads_back_from_its_word() {
        for frame in NoteFrame::ALL {
            assert_eq!(NoteFrame::from_word(frame.as_str()), Some(frame));
        }
        for size in NoteSize::ALL {
            assert_eq!(NoteSize::from_word(size.as_str()), Some(size));
        }
        assert_eq!(NoteFrame::from_word("dashed"), None);
        assert_eq!(
            NoteSize::from_word("Fit_Text"),
            None,
            "words are lower case"
        );
    }

    #[test]
    fn the_box_follows_the_size_section() {
        let fit = RichNoteLayout {
            padding_mm: 1.0,
            ..RichNoteLayout::default()
        };
        assert_eq!(fit.box_size_mm((10.0, 5.0)), (12.0, 7.0));
        assert_eq!(fit.text_width_mm(), None);
        let width = RichNoteLayout {
            size: NoteSize::FixedWidth,
            width_mm: 40.0,
            padding_mm: 1.0,
            ..RichNoteLayout::default()
        };
        assert_eq!(width.text_width_mm(), Some(38.0));
        assert_eq!(width.box_size_mm((38.0, 9.0)), (40.0, 11.0));
        assert!(
            !width.clips((38.0, 900.0)),
            "the height grows with the text"
        );
        let fixed = RichNoteLayout {
            size: NoteSize::FixedBox,
            ..RichNoteLayout::default()
        };
        assert_eq!(
            fixed.box_size_mm((1.0, 1.0)),
            (DEFAULT_NOTE_WIDTH_MM, DEFAULT_NOTE_HEIGHT_MM),
            "an unset box takes the defaults"
        );
        assert!(fixed.clips((1.0, DEFAULT_NOTE_HEIGHT_MM + 1.0)));
        assert!(!fixed.clips((1.0, DEFAULT_NOTE_HEIGHT_MM - 1.0)));
    }

    #[test]
    fn only_a_changed_layout_is_not_default() {
        assert!(RichNoteLayout::default().is_default());
        let border = RichNoteLayout {
            frame: NoteFrame::Border,
            ..RichNoteLayout::default()
        };
        assert!(!border.is_default());
    }
}
