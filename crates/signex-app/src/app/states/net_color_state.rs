pub struct NetColorState {
    /// Net-color overrides keyed by net-label text. Superseded by the
    /// per-wire `wire_color_overrides` map below which the Active-Bar
    /// net-colour flood populates; kept here so a future net-name
    /// palette can cross-reference it without another round-trip.
    #[allow(dead_code)]
    pub colors_by_net: std::collections::HashMap<String, signex_types::theme::Color>,
    /// F5 Net Color palette state — open flag and transient edit buffer.
    pub palette_open: bool,
    /// Net-color override armed from the Active Bar palette. When Some,
    /// the cursor turns into a paint-bucket over the canvas and the
    /// next click on a wire floods that color across every connected
    /// wire. Cleared after the click applies, or by Escape. Colors are
    /// render-time only — they do NOT write back to the .snxsch.
    pub pending_color: Option<signex_types::theme::Color>,
    /// Per-wire color overrides keyed by wire uuid. Populated by the
    /// net-color click; consulted when drawing wires. Not serialised.
    pub wire_color_overrides:
        std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    /// App-level undo stack for net-color floods. Each entry is the
    /// full `wire_color_overrides` map captured before an action —
    /// popping one restores the previous state. This is separate from
    /// the engine's undo because net colours are render-only and
    /// shouldn't mix with document mutations.
    pub undo: Vec<std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>>,
    /// Custom net-color picker state. When `show = true`, a floating
    /// picker appears anchored to the Active Bar button; `draft` is
    /// the user's pending pick — committed on OK.
    pub custom: super::NetColorCustomState,
}
