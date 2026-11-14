//! PCB canvas message family.

use std::sync::Arc;

/// What a design-rule check run came back with (#2222): the report and the
/// net names its rows show, or why it could not run.
pub type DrcRunResult = Result<
    (
        signex_drc::DrcReport,
        std::collections::BTreeMap<u32, String>,
    ),
    String,
>;

use signex_types::pcb_layer::PcbLayer;

/// Board-view events, namespaced under `Message::Pcb` (ADR-0001 D3) and
/// routed to `dispatch_pcb_message`.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbMsg {
    /// The user asked to show or hide one net's ratsnest lines.
    RatsnestNetVisibilityToggled(u32),
    /// A design-rule check of the board in view was asked for (the
    /// `run_drc` command or the Messages panel button).
    DrcRequested,
    /// The check begun as run `generation` finished; `loose` when the board
    /// was in no open project.
    DrcFinished {
        generation: u64,
        loose: bool,
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
    /// The turn of the board view on screen (#2316), from a command: it acts
    /// on the canvas of the window the command runs in.
    ViewRotation(ViewRotationMsg),
    /// The turn of the board view of `window` (its status bar readout).
    ViewRotationInWindow {
        window: iced::window::Id,
        event: ViewRotationMsg,
    },
    /// Board sketch mode of the main window's board (#2234).
    Sketch(BoardSketchMsg),
    /// Tools ▸ Re-Annotate Board and its dialog (#1312).
    Reannotate(crate::app::BoardReannotateMsg),
    /// Back Annotate and its dialog (#1312).
    BackAnnotate(crate::app::BackAnnotateMsg),
    /// The PCB Active Bar's dropdowns (#2352).
    Bar(PcbBarMsg),
    /// Swapping the footprint of placed components (#2366).
    FootprintSwap(FootprintSwapMsg),
}

/// Swapping the library footprint of components placed on a board (#2366).
/// Each step names the board and footprints it is for, so a selection change
/// in between cannot send the swap to other components.
#[derive(Debug, Clone, PartialEq)]
pub enum FootprintSwapMsg {
    /// `change_footprint` ran (the board menu, the palette): the footprint
    /// picker opens for the footprints selected on the board in view.
    Requested,
    /// The picker picked `footprint` for them.
    Picked {
        board: std::path::PathBuf,
        footprints: Vec<uuid::Uuid>,
        footprint: signex_library::PrimitiveRef,
    },
    /// The picked footprint was read off the update thread, in its board
    /// form, or why it could not be.
    Resolved {
        board: std::path::PathBuf,
        footprints: Vec<uuid::Uuid>,
        result: Result<Box<signex_types::pcb::Footprint>, String>,
    },
}

/// A dropdown of the PCB Active Bar (#2352).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcbBarMenu {
    /// Move and transform the board selection.
    Move,
    /// Turn the board view.
    View,
}

/// What the user did on the PCB Active Bar's dropdowns (#2352). Its buttons
/// run their commands through the menu bar's own message.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbBarMsg {
    /// A group button was right-clicked: its dropdown opens, or closes when
    /// it is the open one.
    MenuToggled(PcbBarMenu),
    /// The user clicked outside the open dropdown.
    MenuDismissed,
    /// A dropdown row was picked: the dropdown closes and the command runs
    /// as its menu row runs it.
    CommandPicked(crate::keymap::AppCommandId),
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
    /// The user asked to show or hide the board origin marker (#2351).
    OriginMarkerVisibilityToggled,
    /// The user stepped the active layer to the next or previous one of the
    /// board's layers (`+`, `-`, the status bar pill).
    ActiveLayerStepped { forward: bool },
    /// The user stepped the active layer to the next or previous signal
    /// layer (`*`).
    SignalLayerStepped { forward: bool },
    /// The user made one layer the active one (a tab under the canvas or a
    /// Layers panel row).
    ActiveLayerPicked(PcbLayer),
    /// The user turned single-layer mode on or off (Shift+S, the Layers
    /// panel button).
    SingleLayerModeToggled,
    /// The user showed or hid every layer of one Layers panel group.
    LayerGroupVisibilitySet {
        group: signex_renderer::pcb::LayerGroup,
        visible: bool,
    },
    /// The user asked to show or hide one layer (its eye in the Layers panel).
    LayerVisibilityToggled(PcbLayer),
    /// The user asked to show or hide the active layer.
    ActiveLayerVisibilityToggled,
    /// The user asked to show or hide the Multi-Layer row: vias and
    /// through-hole pads (its eye in the Layers panel).
    MultiLayerVisibilityToggled,
    /// The user asked to show every layer again.
    AllLayersShown,
}

/// What the user did about the turn of a board view on screen (#2316). View
/// state like pan and zoom: never a board edit, no undo step, no dirty mark.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewRotationMsg {
    /// The view was turned by this many degrees, clockwise on screen
    /// (negative: counter-clockwise).
    TurnedBy(f32),
    /// The view was set to this angle, clockwise degrees.
    AngleSet(f32),
    /// The angle readout in the status bar was clicked: its value field
    /// opens, or closes when open.
    AngleEntryToggled,
    /// The text in that value field changed.
    AngleEntryEdited(String),
    /// Enter in that value field: a number sets the angle and closes it.
    AngleEntrySubmitted,
}

/// The status bar's angle field while it is open (#2316): the window it is
/// in and what has been typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewAngleEntry {
    pub window: iced::window::Id,
    pub text: String,
}
