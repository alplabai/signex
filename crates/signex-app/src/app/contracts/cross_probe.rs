//! Cross probe between the schematic and the board (#1312).

/// Cross Probe and Cross Select Mode events, namespaced under
/// `Message::CrossProbe` (ADR-0001 D3) and routed to
/// `dispatch_cross_probe_message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossProbeMsg {
    /// Cross Probe was asked for (the `cross_probe` command): show the
    /// selection's counterpart on the other side, opening it if needed.
    ProbeRequested,
    /// Cross Select Mode was switched (the `toggle_cross_select_mode`
    /// command).
    CrossSelectModeToggled,
}
