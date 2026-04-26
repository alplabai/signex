//! Symbol-tab side of the "Generate from datasheet PDF" wizard.
//!
//! Wraps `signex_library::ai_stub::extract_pinout` and packages the
//! result in a small UI-friendly struct so the modal preview pane can
//! render confidence + pin list without leaking the wire format.
//!
//! No network calls. No LLM. The heuristic is implemented entirely
//! inside `signex-library`.

use signex_library::{PinGuess, PinoutGuess, extract_pinout};

use super::state::PinKind;

/// One pin row as it appears in the preview list.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewPin {
    pub number: String,
    pub name: String,
    pub kind: PinKind,
}

impl From<PinGuess> for PreviewPin {
    fn from(g: PinGuess) -> Self {
        Self {
            number: g.number,
            name: g.name,
            kind: PinKind::from_ai_stub(&g.kind),
        }
    }
}

/// UI-facing wrapper around [`PinoutGuess`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiPinoutPreview {
    pub pins: Vec<PreviewPin>,
    pub confidence: f32,
}

impl AiPinoutPreview {
    /// Run the heuristic over `pdf_bytes` and return a UI-friendly
    /// preview. Returns an empty preview with `confidence == 0.0`
    /// when the bytes are not a valid PDF or carry no pin-table rows.
    pub fn from_pdf(pdf_bytes: &[u8]) -> Self {
        Self::from_guess(extract_pinout(pdf_bytes))
    }

    pub fn from_guess(guess: PinoutGuess) -> Self {
        Self {
            pins: guess.pins.into_iter().map(PreviewPin::from).collect(),
            confidence: guess.confidence,
        }
    }

    /// Whether the parent UI should warn the user. Mirrors the 0.5
    /// threshold called out in `signex-library/src/ai_stub.rs`.
    pub fn is_low_confidence(&self) -> bool {
        self.confidence < 0.5
    }

    /// Convert into the `(number, name, kind)` triple list used by
    /// [`SymbolDoc::apply_ai_pinout`](super::state::SymbolDoc::apply_ai_pinout).
    pub fn into_apply_list(self) -> Vec<(String, String, PinKind)> {
        self.pins
            .into_iter()
            .map(|p| (p.number, p.name, p.kind))
            .collect()
    }
}
