//! Unit tests for the Symbol-tab.
//!
//! Coverage of the typed `Symbol` primitive helpers (`add_pin`,
//! `move_selected`, `delete_selected`, `apply_ai_pinout`) lives
//! alongside the helpers in `state.rs`. This file keeps a single
//! AI-stub low-confidence assertion to ensure the bridge between
//! the AI guess and the typed primitive still flags suspicious
//! heuristics.

use super::ai_stub::AiPinoutPreview;

#[test]
fn ai_stub_default_preview_is_low_confidence() {
    let p = AiPinoutPreview::default();
    assert!(p.is_low_confidence());
}
