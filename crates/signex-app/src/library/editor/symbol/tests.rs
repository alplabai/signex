//! Unit tests for the Symbol-tab — WS-F.
//!
//! Pre-refactor tests exercised the `SymbolDoc::parse / to_sexpr`
//! round-trip; WS-F replaced that with direct `Symbol` primitive
//! manipulation. Coverage now lives alongside the helpers in `state.rs`
//! (`add_pin`, `move_selected`, `delete_selected`, `apply_ai_pinout`).
//! This file keeps a single AI-stub low-confidence assertion to ensure
//! the bridge between the AI guess and the typed primitive still
//! flags suspicious heuristics.

use super::ai_stub::AiPinoutPreview;

#[test]
fn ai_stub_default_preview_is_low_confidence() {
    let p = AiPinoutPreview::default();
    assert!(p.is_low_confidence());
}
