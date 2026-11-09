//! PCB canvas message family.

/// Board-view events, namespaced under `Message::Pcb` (ADR-0001 D3) and
/// routed to `dispatch_pcb_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbMsg {
    /// The user asked to show or hide one net's ratsnest lines.
    RatsnestNetVisibilityToggled(u32),
    /// The PCB canvas of the undocked window `window` reported `msg`: it
    /// acts on that window's board and selection (#2240).
    CanvasInWindow {
        window: iced::window::Id,
        msg: crate::pcb_canvas::PcbCanvasMsg,
    },
}
