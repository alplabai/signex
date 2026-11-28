//! The markup a text note's text is written in (#1114, mockup row 203).
//! Stored beside the notes, not in them: `docs/architecture/snxsch-note-format.md`.

use serde::{Deserialize, Serialize};

/// How a text note's text is read. `Plain` is the note of mockup row 47,
/// drawn through the Signex markup; `Typst` is a rich note whose text is
/// Typst source. Switching between them never changes the text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextNoteFormat {
    #[default]
    Plain,
    Typst,
}

impl TextNoteFormat {
    /// The word a file, an operand and an answer write.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Typst => "typst",
        }
    }

    /// The format `word` names; `None` for a word this build does not know.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "plain" => Some(Self::Plain),
            "typst" => Some(Self::Typst),
            _ => None,
        }
    }

    /// Whether this is the default, which no file writes.
    #[must_use]
    pub fn is_plain(&self) -> bool {
        *self == Self::Plain
    }
}

#[cfg(test)]
mod tests {
    use super::TextNoteFormat;

    #[test]
    fn every_format_reads_back_from_its_word() {
        for format in [TextNoteFormat::Plain, TextNoteFormat::Typst] {
            assert_eq!(TextNoteFormat::from_word(format.as_str()), Some(format));
        }
        assert_eq!(TextNoteFormat::from_word("markdown"), None);
        assert_eq!(
            TextNoteFormat::from_word("Typst"),
            None,
            "words are lower case"
        );
        assert_eq!(TextNoteFormat::default(), TextNoteFormat::Plain);
    }
}
