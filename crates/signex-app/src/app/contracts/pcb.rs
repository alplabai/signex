//! PCB canvas message family.

use std::sync::Arc;

/// What a design-rule check run came back with (#2222): the report and the
/// net names its rows show, or why it could not run.
pub type DrcRunResult =
    Result<(pcb_geom::DrcReport, std::collections::BTreeMap<u32, String>), String>;

/// Board-view events, namespaced under `Message::Pcb` (ADR-0001 D3) and
/// routed to `dispatch_pcb_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbMsg {
    /// The user asked to show or hide one net's ratsnest lines.
    RatsnestNetVisibilityToggled(u32),
    /// A design-rule check of the board in view was asked for (the
    /// `run_drc` command or the Messages panel button).
    DrcRequested,
    /// The check begun as run `generation` finished.
    DrcFinished {
        generation: u64,
        result: Arc<DrcRunResult>,
    },
    /// A violation row (by its position in the report) was clicked.
    DrcViolationPicked(usize),
    /// The PCB canvas of the undocked window `window` reported `msg`: it
    /// acts on that window's board and selection (#2240).
    CanvasInWindow {
        window: iced::window::Id,
        msg: crate::pcb_canvas::PcbCanvasMsg,
    },
}
