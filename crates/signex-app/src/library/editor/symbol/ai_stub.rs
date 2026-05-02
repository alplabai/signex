//! AI pinout preview — minimal stub kept for the `ai_preview` field on
//! `SymbolEditorState`. The full extraction pipeline (`from_pdf` /
//! `from_guess` / `into_apply_list`) is a v0.9-refactor-3 follow-up;
//! wire it when the AI wizard modal lands.

/// UI-facing wrapper around a future AI pinout guess. Holds `confidence`
/// so the caller can warn when the heuristic is not reliable.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiPinoutPreview {
    pub confidence: f32,
}

impl AiPinoutPreview {
    /// Whether the parent UI should warn the user. Mirrors the 0.5
    /// threshold called out in `signex-library/src/ai_stub.rs`.
    pub fn is_low_confidence(&self) -> bool {
        self.confidence < 0.5
    }
}
