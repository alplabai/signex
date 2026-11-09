//! PCB canvas message family.

use std::sync::Arc;

/// What a design-rule check run came back with (#2222): the report and the
/// net names its rows show, or why it could not run.
pub type DrcRunResult =
    Result<(pcb_geom::DrcReport, std::collections::BTreeMap<u32, String>), String>;

use signex_types::pcb_layer::{CopperLayer, PcbLayer};

/// Board-view events, namespaced under `Message::Pcb` (ADR-0001 D3) and
/// routed to `dispatch_pcb_message`.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// The view of the main window's board changed (#2223).
    View(PcbViewMsg),
    /// The view of the board an undocked window shows changed: a layer key
    /// or menu row used in that window.
    ViewInWindow {
        window: iced::window::Id,
        event: PcbViewMsg,
    },
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
