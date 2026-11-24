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
    /// The board 3D view of the main window (#2390).
    Board3d(Board3dMsg),
    /// Board sketch mode of the main window's board (#2234).
    Sketch(BoardSketchMsg),
    /// Tools ▸ Re-Annotate Board and its dialog (#1312).
    Reannotate(crate::app::BoardReannotateMsg),
    /// File ▸ Fabrication Outputs and its dialog (#2388).
    FabOutputs(crate::app::FabOutputsMsg),
    /// Tools ▸ Panelize Board and Remove Panel (#2410).
    Panel(crate::app::BoardPanelMsg),
    /// Back Annotate and its dialog (#1312).
    BackAnnotate(crate::app::BackAnnotateMsg),
    /// Compare Board with Schematic and its dialog (#2462).
    CompareBoard(crate::app::CompareBoardMsg),
    /// Board Reports (#2404) and Board Print / PDF (#2402).
    Output(crate::app::BoardOutputMsg),
    /// The PCB Active Bar's dropdowns (#2352).
    Bar(PcbBarMsg),
    /// A board placement tool was picked (its command ran, #2373): it is
    /// armed on the board in view of the window the command ran in.
    PlaceToolPicked(crate::app::pcb_tools::BoardPlaceTool),
    /// `cycle_route_mode` ran (#2389): Interactive Routing switches between
    /// walking around other nets and pushing their tracks aside.
    RouteModeStepped,
    /// `cycle_arc_mode` ran (#2373): the Arc tool draws its next arc by
    /// centre, by edge or at any angle.
    ArcModeStepped,
    /// `repour_selected` or `repour_all` ran (#2216): the pour fills of the
    /// board in view are computed again and Messages says what the selected
    /// pours (`selected`) or all of them came to.
    RepourRequested { selected: bool },
    /// Swapping the footprint of placed components (#2366).
    FootprintSwap(FootprintSwapMsg),
    /// The right-click menu of a layer tab (#2379).
    LayerTabMenu(super::LayerTabMenuMsg),
    /// The menu of the layer set button (#2394).
    LayerSetMenu(super::LayerSetMenuMsg),
    /// The layer strip's per-user settings changed (#2394).
    LayerStrip(super::LayerStripMsg),
    /// A saved layer set was saved, shown or deleted (#2394).
    LayerSet(super::LayerSetMsg),
    /// Lock or unlock the tracks and vias selected on the board in view was
    /// asked for (`lock_board_items`, `unlock_board_items`, #2407).
    LockRequested { locked: bool },
    /// A net of the board in view picked or highlighted (#2403).
    Net(BoardNetMsg),
    /// A board Tools edit was asked for (#2408, #2409).
    Tools(BoardToolsMsg),
    /// DXF import onto the board in view (#2410).
    DxfImport(DxfImportMsg),
    /// Length tuning on the board in view (#2389).
    Tune(TuneMsg),
    /// Pin and gate swap within a component (#2410).
    PartSwap(PartSwapMsg),
    /// Design reuse blocks of the project (#2410).
    ReuseBlock(crate::app::board_reuse::ReuseBlockMsg),
    /// A room of the board in view was edited in Room Properties (#2457).
    Room(crate::app::rooms::RoomMsg),
    /// Create Rooms from Sheets and its dialog (#2457).
    CreateRooms(crate::app::rooms::CreateRoomsMsg),
    /// Edit ▸ Origin on the board in view (2026-11-18).
    Origin(BoardOriginMsg),
}

/// Edit ▸ Origin (2026-11-18): where the board in view counts from. Each
/// change is one `SetBoardOptions` undo step on `BoardOptions::origin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardOriginMsg {
    /// Set (`set_board_origin`): the next click on the board sets the
    /// origin there.
    SetPicked,
    /// Reset (`reset_board_origin`): the origin goes to the bottom-left of
    /// the board outline, or of the board area without one.
    ResetPicked,
    /// Snap to Outline Corner (`snap_board_origin_to_corner`): the origin
    /// goes to the outline corner nearest the pointer.
    SnapToCornerPicked,
}

/// Net actions on the board in view (#2403, #2405): the board menu's and the
/// palette's rows act on the net of the selection or of what is under the
/// cursor; a Net Inspector row names its net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoardNetMsg {
    /// `select_board_net` ran, or a Net Inspector row's Select was pressed:
    /// every track, via, pad, copper arc, free pad and drawn area of the net
    /// is selected.
    SelectRequested(Option<String>),
    /// `highlight_board_net` ran, or a Net Inspector row's Highlight was
    /// pressed: the net is drawn bright, or no longer when it already is.
    HighlightRequested(Option<String>),
}

/// The board Tools edits (#2408, #2409), each asked for by its command on
/// the board in view and acting on its selection. Each finished edit is one
/// undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardToolsMsg {
    /// `stitch_vias`: the selected areas, or every pour, filled with vias of
    /// their net.
    StitchRequested,
    /// `add_teardrops`: on the nets of the selection, or every net.
    TeardropsAddRequested,
    /// `remove_teardrops`: of the nets of the selection, or every net.
    TeardropsRemoveRequested,
    /// `gloss_selected_routes`: the selected tracks.
    GlossRequested,
    /// `board_shape_from_selection`: the selected lines close the outline.
    BoardShapeRequested,
    /// `keepout_from_selection`: the selected area or closed lines.
    KeepoutRequested,
    /// `arrange_in_box`: the box tool is armed for the selected components;
    /// its two corner clicks arrive as board canvas clicks.
    ArrangeRequested,
}

/// Length tuning asked for on the board in view (#2389), from the PCB
/// Active Bar's Tune group or the palette. Each acts on the selected tracks'
/// nets and lands as one undo step, or says in Messages why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuneMsg {
    /// `tune_net_length` ran: the selected tracks' nets are lengthened to
    /// the longest of them.
    NetLengthRequested,
    /// `tune_diff_pair` ran: the shorter net of the selected track's pair is
    /// lengthened to the other's length.
    DiffPairRequested,
}

/// Pin and gate swap within a component on a board (#2410).
#[derive(Debug, Clone, PartialEq)]
pub enum PartSwapMsg {
    /// `swap_pins` or `swap_gates` ran without operands (the board menu, the
    /// palette, a key): the two pads selected on the board in view name the
    /// two pins, or the two gates they belong to.
    Requested(signex_engine::pcb::pad_swap::SwapKind),
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
    /// Align the board selection (#2373).
    Align,
    /// The object kinds a click or a box picks (#2373).
    Filter,
    /// The rows of a tool group (#2392).
    Group(crate::app::pcb_bar_groups::PcbBarGroup),
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
    /// A bar button took a right press and toggles `menu`; the release of
    /// that press stays off the canvas.
    MenuRightPressed(PcbBarMenu),
    /// A right press outside the open dropdown closed it; its release stays
    /// off the canvas.
    FlyoutRightPressed,
    /// A part of the bar or of its open dropdown that opens no menu took a
    /// right press: nothing changes, and its release stays off the canvas.
    RightPressTaken,
    /// A dropdown row was picked: the dropdown closes and the command runs
    /// as its menu row runs it.
    CommandPicked(crate::keymap::AppCommandId),
}

/// What the user did about board sketch mode (#2234). The drawing itself
/// arrives as the sketch editor's own messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSketchMsg {
    /// `board_sketch` ran: enter sketch mode, or leave it when on (as
    /// Bake does).
    ModeToggled,
    /// Bake (the mode bar's primary button): commit any change to the
    /// board as one board command, then leave. A refused commit keeps the
    /// mode on (#2518).
    BakeRequested,
    /// Bake preview, or Back to sketch: show or hide what a bake writes.
    BakePreviewToggled,
    /// Discard changes (or Esc): leave without committing, asking first
    /// when the sketch has changes the board has not taken.
    DiscardRequested,
    /// The discard was confirmed: leave; the board keeps what it had.
    DiscardConfirmed,
    /// The discard was cancelled: keep editing.
    DiscardCancelled,
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
    /// board's layers (`+`, `-`, View ▸ Next / Previous Layer).
    ActiveLayerStepped { forward: bool },
    /// The user stepped the active layer to the next or previous signal
    /// layer (`*`).
    SignalLayerStepped { forward: bool },
    /// The user made one layer the active one (a tab under the canvas, a
    /// Layers panel row or a row of the status bar's Active layer popover).
    ActiveLayerPicked(PcbLayer),
    /// The user turned single-layer mode on or off (Shift+S, the View
    /// Options switch).
    SingleLayerModeToggled,
    /// The user asked to show or hide one layer (its eye in the Layers panel).
    LayerVisibilityToggled(PcbLayer),
    /// The user asked to show or hide the active layer.
    ActiveLayerVisibilityToggled,
    /// The user asked to show or hide the Multi-Layer row: vias and
    /// through-hole pads (its eye in the Layers panel).
    MultiLayerVisibilityToggled,
    /// The user asked to show every layer again.
    AllLayersShown,
    /// The user hid one layer (Hide this layer on its tab, #2379).
    LayerHidden(PcbLayer),
    /// The user asked to see one layer alone (Show only this layer on its
    /// tab, #2379): it becomes the active layer and every other is hidden.
    OnlyLayerShown(PcbLayer),
    /// The user highlighted one layer, or ended its highlight (#2394):
    /// every other layer is drawn dimmed.
    LayerHighlightToggled(PcbLayer),
    /// The user showed or hid every layer of one kind (Hide layers ▸ and
    /// Show layers ▸ on a tab, #2394).
    LayerKindVisibilitySet {
        kind: signex_renderer::pcb::LayerKind,
        visible: bool,
    },
    /// The user picked a built-in layer set (the layer set button, #2394).
    BuiltinLayerSetShown(signex_renderer::pcb::BuiltinLayerSet),
    /// The user flipped the board view: seen from below or above again
    /// (#2394).
    BoardFlipToggled,
    /// The user showed or hid one layer (an eye of the Layers panel,
    /// `set_layer_visible`, #2363).
    LayerVisibilitySet { layer: PcbLayer, visible: bool },
    /// The user gave a layer its own colour, or (`None`) the board
    /// palette's again (a colour chip, Layer colour... and Reset layer
    /// colour on a tab, `set_layer_color`). Saved with the board as one undo
    /// step (2026-11-18).
    LayerColourSet {
        layer: PcbLayer,
        colour: Option<[u8; 3]>,
    },
    /// Reset all layer colours on a tab (`reset_layer_colors`): every layer
    /// takes the board palette's colour again, as one undo step.
    LayerColoursReset,
    /// The Layers panel lists only the layers the board uses, or every
    /// layer again (Used On, #2363).
    UsedLayersOnlySet(bool),
    /// The user turned Show only unrouted on or off (the Unrouted popover,
    /// mockup row 164): view state, never a board edit or an undo step.
    OnlyUnroutedToggled,
    /// The user showed or hid one kind of object; `None`: every kind the
    /// view can hide (View Options, #2363).
    ObjectVisibilitySet {
        kind: Option<signex_renderer::pcb::BoardObject>,
        visible: bool,
    },
    /// The user set how transparent one kind of object is drawn, percent
    /// (View Options, #2363).
    ObjectTransparencySet {
        kind: signex_renderer::pcb::BoardObject,
        percent: u8,
    },
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

/// What the user did in the board 3D view (#2390). View state like pan and
/// zoom: never a board edit, no undo step, no dirty mark.
#[derive(Debug, Clone, PartialEq)]
pub enum Board3dMsg {
    /// `view_board_3d` ran (View ▸ 3D View, its key, the palette): the
    /// board in view shows in 3D, or flat again.
    Toggled,
    /// The toolbar's 2D segment: the board shows flat again.
    LeaveRequested,
    /// A viewpoint button was picked: the view looks from there on the side
    /// shown, framed.
    PresetPicked(signex_gfx::camera3d::ViewPreset),
    /// The Fit button: the board is framed from where it is seen now.
    FitRequested,
    /// The Reset button: Iso on the side shown, framed. Flip, explode and
    /// lighting stay.
    ResetRequested,
    /// The Flip button: the board turns over in one animated step.
    FlipRequested,
    /// A frame was drawn while the flip turns or a screenshot waits for a
    /// frame without the toolbar.
    AnimationTicked(std::time::Instant),
    /// The Explode button: its popover opens, or closes.
    ExplodePopoverToggled,
    /// The explode slider moved to this percent (0 assembled, 100 apart).
    ExplodeAmountChanged(u8),
    /// "Separate copper and mask" was ticked or cleared.
    ExplodeLayersToggled(bool),
    /// "Lift components" was ticked or cleared.
    ExplodePartsToggled(bool),
    /// The lighting button: the next lighting preset, and its toast.
    LightingCycled,
    /// The lighting toast of this generation timed out.
    LightingToastExpired(u64),
    /// The screenshot button's chevron: its menu opens, or closes.
    ScreenshotMenuToggled,
    /// Esc closed the open Explode popover or screenshot menu.
    PopoverDismissed,
    /// The screenshot button, or Copy to clipboard in its menu.
    ScreenshotCopyRequested,
    /// Save PNG… in the screenshot menu: ask where.
    ScreenshotSaveRequested,
    /// The save dialog answered; `None` is a cancel.
    ScreenshotSavePicked(Option<std::path::PathBuf>),
    /// Save with transparent backdrop… in the screenshot menu: ask where.
    ScreenshotTransparentSaveRequested,
    /// The transparent save's dialog answered; `None` is a cancel.
    ScreenshotTransparentSavePicked(Option<std::path::PathBuf>),
    /// The view was captured, cropped to the view; `None` when the window
    /// had nothing to show. `save_to` is the PNG to write; `None` copies it
    /// to the clipboard.
    ScreenshotTaken {
        save_to: Option<std::path::PathBuf>,
        shot: Option<Arc<Board3dShot>>,
    },
    /// The clipboard took the screenshot, or refused it and why.
    ScreenshotCopied(Result<(), iced::clipboard::Error>),
    /// The PNG at `path` was written, or why it was not.
    ScreenshotSaved {
        path: std::path::PathBuf,
        error: Option<String>,
    },
    /// The model-unread toast was dismissed; its Messages lines stay.
    UnreadNoticeDismissed,
    /// A redraw found the view at another window position, in logical
    /// pixels: where a screenshot is cropped.
    ViewportPlaced([f32; 2]),
    /// A redraw found the view at a size it had not reported, in logical
    /// pixels; the first one frames the view for its real shape.
    ViewportSized([f32; 2]),
    /// The pointer did `gesture` over the view, `viewport` logical pixels.
    Pointer {
        viewport: [f32; 2],
        gesture: Board3dGesture,
    },
    /// The models of the library footprints the parts of `board` came from
    /// were read off the update thread.
    ModelsRead {
        board: std::path::PathBuf,
        read: Arc<Board3dModelsRead>,
    },
}

/// A pointer gesture over the board 3D view, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Board3dGesture {
    /// A left drag turned the view.
    Orbited { dx: f32, dy: f32 },
    /// A right or middle drag, or a Shift left drag, slid the view.
    Panned { dx: f32, dy: f32 },
    /// The wheel moved in (positive) or out by this many steps.
    Zoomed(f32),
    /// A left click at this point of the view.
    Clicked([f32; 2]),
}

/// A screenshot of the board 3D view: RGBA pixels, `size` wide and tall in
/// physical pixels.
#[derive(Clone, PartialEq)]
pub struct Board3dShot {
    pub rgba: Vec<u8>,
    pub size: [u32; 2],
}

impl std::fmt::Debug for Board3dShot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board3dShot")
            .field("bytes", &self.rgba.len())
            .field("size", &self.size)
            .finish()
    }
}

/// The models read for a board's parts, and why the others could not be.
#[derive(Debug, Default, PartialEq)]
pub struct Board3dModelsRead {
    pub models: Vec<(
        signex_types::part::FootprintSourceRef,
        signex_renderer::board3d::ComponentModel,
    )>,
    /// The source that could not be read, as text, and why.
    pub failures: Vec<(String, String)>,
}

/// DXF import onto a board (#2410). Each step names the board and where the
/// drawing goes, so a tab switch while the dialog is open cannot send the
/// import to another board or layer.
#[derive(Debug, Clone, PartialEq)]
pub enum DxfImportMsg {
    /// The command ran without operands: ask for the file. `outline` makes
    /// the largest closed shape the board outline; otherwise the drawing goes
    /// onto the active layer of the board in view.
    Requested { outline: bool },
    /// The file dialog answered; `None` is a cancel.
    Picked {
        board: std::path::PathBuf,
        target: signex_engine::pcb::dxf::DxfTarget,
        path: Option<std::path::PathBuf>,
    },
    /// The file was read off the UI thread, or why it could not be.
    /// `waiting` is the command run with operands that is answered with the
    /// import's summary or refusal; `None` for the file dialog's answer.
    Read {
        board: std::path::PathBuf,
        target: signex_engine::pcb::dxf::DxfTarget,
        path: std::path::PathBuf,
        text: Result<String, String>,
        waiting: Option<crate::app::agent::WaitingCommand>,
    },
    /// The file dialog of a board sketch's Import DXF answered (mockup row
    /// 179c); `None` is a cancel.
    SketchPicked {
        board: std::path::PathBuf,
        path: Option<std::path::PathBuf>,
    },
    /// That file was read off the UI thread, or why it could not be.
    SketchRead {
        board: std::path::PathBuf,
        path: std::path::PathBuf,
        source: Result<std::sync::Arc<signex_engine::pcb::dxf::DxfSource>, String>,
    },
    /// A choice, Import or Cancel in the sketch's Import DXF dialog.
    SketchDialog(crate::app::state::SketchDxfMsg),
}
