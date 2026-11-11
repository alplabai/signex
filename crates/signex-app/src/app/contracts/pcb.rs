//! PCB canvas message family.

use std::sync::Arc;

/// What a design-rule check run came back with (#2222): the report and the
/// net names its rows show, or why it could not run.
pub type DrcRunResult =
    Result<(pcb_geom::DrcReport, std::collections::BTreeMap<u32, String>), String>;

use signex_types::pcb_layer::{CopperLayer, PcbLayer};

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
    /// The view of the main window's board changed (#2223).
    View(PcbViewMsg),
    /// The view of the board an undocked window shows changed: a layer key
    /// or menu row used in that window.
    ViewInWindow {
        window: iced::window::Id,
        event: PcbViewMsg,
    },
    /// Board sketch mode of the main window's board (#2234).
    Sketch(BoardSketchMsg),
    /// Tools ▸ Re-Annotate Board and its dialog (#1312).
    Reannotate(crate::app::BoardReannotateMsg),
}

/// What the user did about board sketch mode (#2234). The drawing itself
/// arrives as the sketch editor's own messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSketchMsg {
    /// `board_sketch` ran: enter sketch mode, or leave it when on.
    ModeToggled,
    /// Leave (the bar's button or Esc): commit any change, then leave. A
    /// refused commit keeps the mode on.
    LeaveRequested,
    /// Apply: commit the changes and stay in the mode.
    ApplyRequested,
    /// Discard: leave without committing; the board keeps what it had.
    DiscardRequested,
}

/// What the user changed about what a board view shows (#2223): never the
/// board itself. The Layers panel sends these and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcbViewMsg {
    /// The user asked to show or hide the whole ratsnest.
    RatsnestVisibilityToggled,
    /// The user stepped the active copper layer to the next or previous one.
    ActiveLayerStepped { forward: bool },
    /// The user made one copper layer the active one (a Layers panel row).
    ActiveLayerPicked(CopperLayer),
    /// The user asked to show or hide one layer (its eye in the Layers panel).
    LayerVisibilityToggled(PcbLayer),
    /// The user asked to show or hide the active layer.
    ActiveLayerVisibilityToggled,
    /// The user asked to show every layer again.
    AllLayersShown,
}
