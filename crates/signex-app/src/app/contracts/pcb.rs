//! PCB canvas message family.

/// Board-view events, namespaced under `Message::Pcb` (ADR-0001 D3) and
/// routed to `dispatch_pcb_message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PcbMsg {
    /// The user asked to show or hide one net's ratsnest lines.
    RatsnestNetVisibilityToggled(u32),
}
