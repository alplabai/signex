use crate::exec::change_component::ComponentReplacement;
use signex_types::schematic::{
    AnnotateOrder, Bus, BusEntry, ChildSheet, HAlign, Junction, Label, LibSymbol, NoConnect,
    SchematicSheet, SelectedItem, SheetPin, StrokeColor, Symbol, TextNote, Wire,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPort {
    pub name: String,
    pub direction: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorAxis {
    Horizontal,
    Vertical,
}

/// What [`Command::TransformSelection`] does to a selection. Directions are
/// on screen (y-down). `Mirror(Vertical)` flips up/down (what toggles a
/// symbol's `mirror_x`); `Mirror(Horizontal)` flips left/right (`mirror_y`),
/// the same pairing as [`Command::MirrorSelection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionTransform {
    RotateCounterclockwise,
    RotateClockwise,
    Mirror(MirrorAxis),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    Label(Uuid),
    TextNote(Uuid),
    SymbolReference(Uuid),
    SymbolValue(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolTextField {
    Reference,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    ReplaceDocument,
    MoveSelection,
    MoveEach,
    RotateSelection,
    MirrorSelection,
    TransformSelection,
    DeleteSelection,
    UpdateText,
    UpdateLabelProps,
    SetSymbolRotation,
    SetSymbolLocked,
    SetItemLocked,
    SetSymbolDnp,
    SetSymbolBodyStyle,
    UpdateSymbolTextSize,
    UpdateSymbolLibId,
    UpdateSymbolFootprint,
    UpdateSymbolFields,
    PlaceWireSegment,
    BreakWire,
    PlaceBus,
    PlaceLabel,
    PlaceSymbol,
    RegisterLibSymbol,
    ReplaceLibSymbol,
    ChangeComponent,
    PlaceChildSheet,
    PlaceSheetPin,
    PlaceJunction,
    PlaceNoConnect,
    PlaceNoErcDirective,
    PlaceBusEntry,
    PlaceTextNote,
    PlaceSchDrawing,
    UpdateSchDrawing,
    UpdateChildSheetStyle,
    AnnotateAll,
    MoveSymbolAbsolute,
    ReorderObjects,
    ReconcileChildSheetPins,
    RenameChildSheetFile,
    ModifyChildSheet,
    SetPaperSize,
    SetSheetFrame,
    SetTitleBlockField,
    SetTextFontFamily,
    SetSymbolDesignatorIn,
    SetDocumentFontFamily,
    ArrangeDrawings,
    SetDrawingPlacement,
}

#[derive(Debug, Clone)]
pub enum Command {
    ReplaceDocument {
        document: SchematicSheet,
    },
    MoveSelection {
        items: Vec<SelectedItem>,
        dx: f64,
        dy: f64,
        /// Rubber-band: wire endpoints sitting on a moved symbol's (or child
        /// sheet's) pins follow them, bending to stay orthogonal, inside
        /// the same history step. `false` is a plain translate that leaves
        /// wires where they are (see `transform::rubber_band`).
        keep_wires: bool,
    },
    /// Move each item by its own delta in one history step (align and
    /// distribute). Rubber-bands like [`Command::MoveSelection`].
    MoveEach {
        moves: Vec<(SelectedItem, f64, f64)>,
        keep_wires: bool,
    },
    /// With `keep_wires` the wires on the turned pins follow, as for
    /// [`Command::MoveSelection`], inside the same history step.
    RotateSelection {
        items: Vec<SelectedItem>,
        angle_degrees: f64,
        keep_wires: bool,
    },
    /// Rubber-bands like [`Command::RotateSelection`].
    MirrorSelection {
        items: Vec<SelectedItem>,
        axis: MirrorAxis,
        keep_wires: bool,
    },
    /// Rotate or mirror any schematic selection 90 degrees / across an axis
    /// through `centre`, as one undo step. Every item's position moves about
    /// the centre; symbols, labels and text notes also update their own
    /// orientation. A selection holding a sheet symbol or sheet pin is
    /// refused whole (`EngineError::SelectionTransformUnsupported`) before
    /// anything is touched; a symbol's ref/value fields travel with their
    /// symbol. Callers pick the centre (see `Engine::selection_centre`) and
    /// choose a pivot that keeps on-grid points on the grid.
    TransformSelection {
        items: Vec<SelectedItem>,
        centre: signex_types::schematic::Point,
        transform: SelectionTransform,
        /// Rubber-bands like [`Command::RotateSelection`].
        keep_wires: bool,
    },
    DeleteSelection {
        items: Vec<SelectedItem>,
    },
    UpdateText {
        target: TextTarget,
        value: String,
    },
    UpdateLabelProps {
        label_id: Uuid,
        font_size_mm: Option<f64>,
        justify: Option<HAlign>,
        rotation_degrees: Option<f64>,
    },
    /// Absolute rotation for a single symbol (used by the properties panel
    /// Rotation dropdown — "0/90/180/270 Degrees"). Distinct from the
    /// `RotateSelection` delta command.
    SetSymbolRotation {
        symbol_id: Uuid,
        rotation_degrees: f64,
    },
    /// Set a symbol's `locked` flag (Properties panel "Locked" checkbox).
    SetSymbolLocked {
        symbol_id: Uuid,
        locked: bool,
    },
    /// Set the `locked` flag of a wire, label, text note or drawing (#1573);
    /// the Properties panel "Locked" checkbox. A symbol uses
    /// `SetSymbolLocked`; any other kind is a no-op.
    SetItemLocked {
        item: SelectedItem,
        locked: bool,
    },
    /// Set a symbol's `dnp` (Do Not Populate) flag (Properties panel "DNP"
    /// checkbox). The BOM exporters read this flag.
    SetSymbolDnp {
        symbol_id: Uuid,
        dnp: bool,
    },
    /// Show the normal or the alternate (De Morgan) body of a symbol's library
    /// symbol. Refused with `EngineError::SymbolBodyPinsConnected` when a wired
    /// pin would be lost, moved or land on a new connection.
    SetSymbolBodyStyle {
        symbol_id: Uuid,
        body_style: signex_types::schematic::BodyStyle,
    },
    /// Set the font size of a symbol's value/reference text property.
    /// `field` is "value" or "reference".
    UpdateSymbolTextSize {
        symbol_id: Uuid,
        field: SymbolTextField,
        font_size_mm: f64,
    },
    UpdateSymbolFootprint {
        symbol_id: Uuid,
        footprint: String,
    },
    /// Change the lib_id reference of a symbol — used by the power-port Style
    /// dropdown to pick a new variant (Bar / Arrow / Wave / Circle / GND …).
    UpdateSymbolLibId {
        symbol_id: Uuid,
        lib_id: String,
    },
    UpdateSymbolFields {
        symbol_id: Uuid,
        reference: String,
        value: String,
        footprint: String,
    },
    /// Set a single custom parameter/field on a symbol. `reference`,
    /// `value`, and `footprint` have dedicated `UpdateSymbolFields`
    /// above — this handles every other key in `symbol.fields`, for
    /// example the Parameter Manager editing `Manufacturer` or `PartNo`.
    /// Empty `value` removes the field.
    SetSymbolField {
        symbol_id: Uuid,
        key: String,
        value: String,
    },
    /// Split one wire in two at `at`, which must lie strictly inside it; the
    /// first half keeps the wire's id, the second gets a new one. A point
    /// off the wire, or at one of its ends, changes nothing. Connectivity is
    /// unchanged and no merge pass runs, so the halves stay apart.
    BreakWire {
        wire: Uuid,
        at: signex_types::schematic::Point,
    },
    PlaceWireSegment {
        wire: Wire,
    },
    PlaceBus {
        bus: Bus,
    },
    PlaceLabel {
        label: Label,
    },
    PlaceSymbol {
        symbol: Symbol,
    },
    /// Add `lib_symbol` to the sheet's `lib_symbols`, keyed by its `id`.
    /// A no-op when that key is already present — the first definition
    /// wins, so re-placing a library symbol never duplicates or replaces
    /// the entry. Run in the same batch as the `PlaceSymbol` that
    /// references it so one undo removes both.
    RegisterLibSymbol {
        lib_symbol: LibSymbol,
    },
    /// Replace the sheet's embedded copy of `lib_symbol.id` with
    /// `lib_symbol` ("Update from library", #912). Refused with
    /// [`EngineError::LibSymbolPinsConnected`] when a pin that has a wire
    /// or no-connect on it would vanish or move off that connection;
    /// nothing is mutated then. One undo step.
    ReplaceLibSymbol {
        lib_symbol: LibSymbol,
        /// `(placed symbol uuid, library_version)` stamps applied in the
        /// same undo step: the version is informational, the content is the
        /// truth (#950).
        library_versions: Vec<(uuid::Uuid, String)>,
    },
    /// Swap the placed symbol `symbol_id` for another library part
    /// ("Change Component...", #1260): register the part's drawing, re-key
    /// the symbol to it and stamp the part's library identity and value.
    /// The designator, position, orientation and parameters stay. One undo
    /// step. It does not refuse over connected pins: the caller shows
    /// [`change_component_preview`] first and the user confirms.
    ChangeComponent {
        symbol_id: Uuid,
        replacement: ComponentReplacement,
    },
    /// Add a hierarchical sheet symbol. Rejected with
    /// [`EngineError::EmptyChildSheetFilename`] when `filename` is blank.
    PlaceChildSheet {
        child_sheet: ChildSheet,
    },
    /// Add a hand-placed entry (pin) to the sheet symbol `sheet_id`. Refused
    /// with a name error when the name is blank or the symbol already has an
    /// entry of that name; an unknown `sheet_id` changes nothing. Undoable.
    PlaceSheetPin {
        sheet_id: Uuid,
        pin: SheetPin,
    },
    PlaceJunction {
        junction: Junction,
    },
    PlaceNoConnect {
        no_connect: NoConnect,
    },
    /// Place a generic No-ERC directive; lands in `no_erc_directives`,
    /// never `no_connects`.
    PlaceNoErcDirective {
        directive: NoConnect,
    },
    PlaceBusEntry {
        bus_entry: BusEntry,
    },
    PlaceTextNote {
        text_note: TextNote,
    },
    /// Append a freeform graphic (line / rect / circle / arc / polyline)
    /// to the sheet's drawings list. Used by the Arc 3-click tool and
    /// the Polyline click-by-click tool.
    PlaceSchDrawing {
        drawing: signex_types::schematic::SchDrawing,
    },
    /// Replace an existing SchDrawing by uuid — used by the drawing
    /// properties panel for per-field edits (angle, radius, vertex
    /// coords, fill, width).
    UpdateSchDrawing {
        drawing: signex_types::schematic::SchDrawing,
    },
    /// Update a hierarchical child sheet's outline / fill / line-width
    /// styling. `stroke_color` and `fill_color` are double-Optioned so
    /// callers can distinguish "don't touch this field" (`None`) from
    /// "reset to theme default" (`Some(None)`) and "use this colour"
    /// (`Some(Some(c))`). `stroke_width` of `None` means leave the
    /// width untouched.
    UpdateChildSheetStyle {
        sheet_id: Uuid,
        stroke_width: Option<f64>,
        stroke_color: Option<Option<StrokeColor>>,
        fill_color: Option<Option<StrokeColor>>,
    },
    /// Auto-annotate every symbol whose reference ends in `?` with a unique
    /// sequential designator per prefix (R?, C?, U?, …). Applied in a
    /// deterministic order so re-running produces the same layout.
    AnnotateAll {
        mode: AnnotateMode,
        /// Visit order; `AnnotateOrder::default()` is rows top to bottom.
        order: AnnotateOrder,
    },
    /// Absolute positioning of a single symbol. Used by the Properties
    /// panel's future X/Y edit fields and by scripted moves; distinct from
    /// MoveSelection which takes a delta.
    MoveSymbolAbsolute {
        symbol_id: Uuid,
        x: f64,
        y: f64,
    },
    /// File-order reorder — moves the given selection to the start or end
    /// of each type vector. The Signex schematic has no explicit z-order
    /// field; render order is the file's vector order, so this command
    /// reshapes that order.
    ReorderObjects {
        items: Vec<SelectedItem>,
        direction: ReorderDirection,
    },
    /// Reconcile parent-sheet symbolic pins for all child sheets that point
    /// to `child_filename`, based on the exposed ports from that child.
    ReconcileChildSheetPins {
        child_filename: String,
        ports: Vec<SheetPort>,
    },
    /// Re-point every hierarchical child-sheet reference whose `filename`
    /// is `from` at `to` — the follow-up to renaming the referenced
    /// `.snxsch` on disk. Undoable like any other edit.
    RenameChildSheetFile {
        from: String,
        to: String,
    },
    /// Rename and / or re-point one placed sheet symbol, by `sheet_id`.
    /// `None` leaves that field untouched. The name must be non-empty and
    /// the file must keep the `.snxsch` extension; both must stay unique
    /// among the document's sheet symbols. Only the reference changes —
    /// no file is created, moved or renamed. Undoable like any other edit.
    ModifyChildSheet {
        sheet_id: Uuid,
        name: Option<String>,
        filename: Option<String>,
    },
    /// Set the sheet's paper size (a named format like "A4"/"A3" — the
    /// Page Options panel is the producer). Persists through
    /// `SchematicSheet.paper_size` and is undoable like any other edit.
    SetPaperSize {
        paper_size: String,
    },
    /// Set the sheet's frame settings: border, title block and zone counts
    /// (Page Options). Zone counts are clamped to `MAX_ZONES`.
    SetSheetFrame {
        settings: signex_types::schematic::SheetFrameSettings,
    },
    /// Set one title-block field of the sheet (`title`, `rev`, `date`,
    /// `company`, `drawn_by`, …; the Properties panel's Title Block section
    /// is the producer). Stored in `SchematicSheet.title_block`; an empty
    /// value removes the key. Undoable like any other edit (#1080).
    SetTitleBlockField {
        key: String,
        value: String,
    },
    /// Set one symbol's designator at one placement of its sheet: the
    /// instance entry for `placement` (#1151). A blank `reference` removes the
    /// entry, so the placement falls back to the symbol's `reference`. The
    /// root chain is not a placement and leaves the document unchanged.
    SetSymbolDesignatorIn {
        symbol: Uuid,
        placement: signex_types::schematic::InstancePath,
        reference: String,
    },
    /// Set (`Some`) or clear (`None`, inherit the sheet default) the font
    /// family of one text item. The name is stored as given; whether the
    /// family is installed is decided at render time and never rewrites it.
    SetTextFontFamily {
        target: TextTarget,
        font_family: Option<String>,
    },
    /// Set or clear the sheet-wide default font family (Document Options).
    SetDocumentFontFamily {
        font_family: Option<String>,
    },
    /// Move the drawings `ids` within the sheet's drawing list, as one undo
    /// step: the selected drawings keep their order among themselves, and an
    /// id that names no drawing is ignored. Unchanged when nothing moves (the
    /// selection is already at that end). See [`DrawingArrange`].
    ArrangeDrawings {
        ids: Vec<Uuid>,
        arrange: DrawingArrange,
    },
    /// Put the drawings `ids` behind or above the circuit (#1583).
    SetDrawingPlacement {
        ids: Vec<Uuid>,
        placement: signex_types::schematic::DrawingPlacement,
    },
}

/// How [`Command::ArrangeDrawings`] moves the selected drawings. The list is
/// painted first to last, so the end of the list is the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawingArrange {
    /// Bring to Front: to the end of the list.
    ToFront,
    /// Send to Back: to the start of the list.
    ToBack,
    /// Bring Forward: past the next unselected drawing toward the front.
    Forward,
    /// Send Backward: past the previous unselected drawing toward the back.
    Backward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReorderDirection {
    /// Move to the end of the vector so it renders on top (Bring To Front).
    ToFront,
    /// Move to the start of the vector so it renders behind (Send To Back).
    ToBack,
    /// Move just after the reference item's slot — renders on top of
    /// the reference but below anything above it.
    JustAbove(Uuid),
    /// Move just before the reference item's slot — renders behind the
    /// reference but on top of anything below it.
    JustBelow(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotateMode {
    /// Only assign a number when the reference ends in `?`.
    Incremental,
    /// Reset every reference to `<prefix>?` then renumber from 1.
    ResetAndRenumber,
    /// Drop every reference number back to `?`. Useful before Reset.
    ResetOnly,
}

impl Command {
    /// Whether executing this command reads the pre-edit sheet beyond
    /// recording it: the move/turn commands check their rubber-banded result
    /// against the nets as they were.
    pub(crate) fn reads_pre_image(&self) -> bool {
        matches!(
            self,
            Self::MoveSelection { .. }
                | Self::MoveEach { .. }
                | Self::RotateSelection { .. }
                | Self::MirrorSelection { .. }
                | Self::TransformSelection { .. }
        )
    }

    pub fn kind(&self) -> CommandKind {
        match self {
            Command::ReplaceDocument { .. } => CommandKind::ReplaceDocument,
            Command::MoveSelection { .. } => CommandKind::MoveSelection,
            Command::MoveEach { .. } => CommandKind::MoveEach,
            Command::RotateSelection { .. } => CommandKind::RotateSelection,
            Command::MirrorSelection { .. } => CommandKind::MirrorSelection,
            Command::TransformSelection { .. } => CommandKind::TransformSelection,
            Command::DeleteSelection { .. } => CommandKind::DeleteSelection,
            Command::UpdateText { .. } => CommandKind::UpdateText,
            Command::UpdateLabelProps { .. } => CommandKind::UpdateLabelProps,
            Command::SetSymbolRotation { .. } => CommandKind::SetSymbolRotation,
            Command::SetSymbolLocked { .. } => CommandKind::SetSymbolLocked,
            Command::SetItemLocked { .. } => CommandKind::SetItemLocked,
            Command::SetSymbolDnp { .. } => CommandKind::SetSymbolDnp,
            Command::SetSymbolBodyStyle { .. } => CommandKind::SetSymbolBodyStyle,
            Command::UpdateSymbolTextSize { .. } => CommandKind::UpdateSymbolTextSize,
            Command::UpdateSymbolLibId { .. } => CommandKind::UpdateSymbolLibId,
            Command::UpdateSymbolFootprint { .. } => CommandKind::UpdateSymbolFootprint,
            Command::UpdateSymbolFields { .. } => CommandKind::UpdateSymbolFields,
            Command::SetSymbolField { .. } => CommandKind::UpdateSymbolFields,
            Command::PlaceWireSegment { .. } => CommandKind::PlaceWireSegment,
            Command::BreakWire { .. } => CommandKind::BreakWire,
            Command::PlaceBus { .. } => CommandKind::PlaceBus,
            Command::PlaceLabel { .. } => CommandKind::PlaceLabel,
            Command::PlaceSymbol { .. } => CommandKind::PlaceSymbol,
            Command::RegisterLibSymbol { .. } => CommandKind::RegisterLibSymbol,
            Command::ReplaceLibSymbol { .. } => CommandKind::ReplaceLibSymbol,
            Command::ChangeComponent { .. } => CommandKind::ChangeComponent,
            Command::PlaceChildSheet { .. } => CommandKind::PlaceChildSheet,
            Command::PlaceSheetPin { .. } => CommandKind::PlaceSheetPin,
            Command::PlaceJunction { .. } => CommandKind::PlaceJunction,
            Command::PlaceNoConnect { .. } => CommandKind::PlaceNoConnect,
            Command::PlaceNoErcDirective { .. } => CommandKind::PlaceNoErcDirective,
            Command::PlaceBusEntry { .. } => CommandKind::PlaceBusEntry,
            Command::PlaceTextNote { .. } => CommandKind::PlaceTextNote,
            Command::PlaceSchDrawing { .. } => CommandKind::PlaceSchDrawing,
            Command::UpdateSchDrawing { .. } => CommandKind::UpdateSchDrawing,
            Command::UpdateChildSheetStyle { .. } => CommandKind::UpdateChildSheetStyle,
            Command::AnnotateAll { .. } => CommandKind::AnnotateAll,
            Command::MoveSymbolAbsolute { .. } => CommandKind::MoveSymbolAbsolute,
            Command::ReorderObjects { .. } => CommandKind::ReorderObjects,
            Command::ReconcileChildSheetPins { .. } => CommandKind::ReconcileChildSheetPins,
            Command::RenameChildSheetFile { .. } => CommandKind::RenameChildSheetFile,
            Command::ModifyChildSheet { .. } => CommandKind::ModifyChildSheet,
            Command::SetPaperSize { .. } => CommandKind::SetPaperSize,
            Command::SetSheetFrame { .. } => CommandKind::SetSheetFrame,
            Command::SetTitleBlockField { .. } => CommandKind::SetTitleBlockField,
            Command::SetTextFontFamily { .. } => CommandKind::SetTextFontFamily,
            Command::SetSymbolDesignatorIn { .. } => CommandKind::SetSymbolDesignatorIn,
            Command::SetDocumentFontFamily { .. } => CommandKind::SetDocumentFontFamily,
            Command::ArrangeDrawings { .. } => CommandKind::ArrangeDrawings,
            Command::SetDrawingPlacement { .. } => CommandKind::SetDrawingPlacement,
        }
    }
}
