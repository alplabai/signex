use crate::exec::change_component::ComponentReplacement;
use signex_types::corner::DragWiringMode;
use signex_types::schematic::{
    AnnotateOrder, Bus, BusEntry, ChildSheet, HAlign, Junction, Label, LibSymbol, NoConnect,
    NoErcDirective, SchematicSheet, SelectedItem, SheetPin, StrokeColor, Symbol, TextNote, VAlign,
    Wire,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetPort {
    pub name: String,
    pub direction: String,
}

/// A sheet entry or port that keeps its place and takes a new name (#1276).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetEntryRename {
    pub from: String,
    pub to: String,
}

/// The entry renames a sync makes on one sheet symbol, by its id (#1276).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntryRenames {
    pub symbol: Uuid,
    pub renames: Vec<SheetEntryRename>,
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

/// A boolean symbol flag with a dedicated edit (`dnp` and `locked` have their
/// own commands).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolFlag {
    /// Listed in the BOM.
    InBom,
    /// Carried to the board by a PCB update.
    OnBoard,
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
    SetSymbolFlag,
    SetWireWidth,
    SetWireColor,
    SetSymbolBodyStyle,
    UpdateSymbolTextSize,
    UpdateSymbolLibId,
    UpdateSymbolFootprint,
    SetSymbolFootprintBinding,
    UpdateSymbolFields,
    PlaceWireSegment,
    BreakWire,
    MoveWireEnd,
    SetRunEnd,
    UpdateTextNoteProps,
    PlaceWireSegmentAsIs,
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
    SetChildSheetInstances,
    ResizeChildSheet,
    SetPaperSize,
    SetSheetFrame,
    SetSheetColor,
    SetPageOrigin,
    SetTitleBlockField,
    SetSheetVariable,
    SetTextFontFamily,
    SetSymbolDesignatorIn,
    SetDocumentFontFamily,
    SetNetColors,
    ArrangeDrawings,
    SetDrawingPlacement,
    PlaceDirective,
    UpdateDirective,
    SetNoErcRules,
    AutoplaceFields,
    PlaceHarnessConnector,
    UpdateHarnessConnector,
    PlaceHarnessEntry,
    PlaceHarnessWire,
    SetHarnessEntryType,
    CreateUnion,
    UpdateUnion,
    SetUnionMembers,
    BreakUnion,
}

/// The text-note properties [`Command::UpdateTextNoteProps`] sets; `None`
/// keeps a property as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextNoteProps {
    pub justify_h: Option<HAlign>,
    pub justify_v: Option<VAlign>,
    /// Stored degrees, counter-clockwise; normalised into `0..360`.
    pub rotation_degrees: Option<f64>,
    /// Must be above zero; a size at or below zero is ignored.
    pub font_size_mm: Option<f64>,
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
        /// sheet's) pins follow them, bending as `wiring` says, inside
        /// the same history step. `false` is a plain translate that leaves
        /// wires where they are (see `transform::rubber_band`).
        keep_wires: bool,
        /// How a rubber-banded wire bends; `Auto` unless the user picked a
        /// mode during a drag.
        wiring: DragWiringMode,
    },
    /// Move each item by its own delta in one history step (align and
    /// distribute). Rubber-bands like [`Command::MoveSelection`]. A
    /// designator or value field whose symbol is also in `moves` travels with
    /// the symbol, by the symbol's delta; its own entry is dropped (#2087).
    MoveEach {
        moves: Vec<(SelectedItem, f64, f64)>,
        keep_wires: bool,
        wiring: DragWiringMode,
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
    /// Set a symbol's BOM / board flag (Properties of a multi-selection).
    SetSymbolFlag {
        symbol_id: Uuid,
        flag: SymbolFlag,
        value: bool,
    },
    /// Set a wire's own stroke width in mm (0.0 = the schematic default). A
    /// net class width still wins when the wire's net has one.
    SetWireWidth {
        wire_id: Uuid,
        width_mm: f64,
    },
    /// Set a wire's own colour; `None` clears it back to the theme wire
    /// colour. The wire colour wins over a per-net colour and a class colour.
    SetWireColor {
        wire_id: Uuid,
        color: Option<signex_types::theme::Color>,
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
    /// Build a library part with another footprint of its row (#1234): set
    /// the footprint name and record the choice in the part's value stamp as
    /// the row value it now agrees with. `binding` is the footprint's
    /// `<library uuid>:<footprint uuid>`, `None` for the row's default. One
    /// undo step; a part with no library row keeps no stamp.
    SetSymbolFootprintBinding {
        symbol_id: Uuid,
        footprint: String,
        binding: Option<String>,
    },
    /// Change the lib_id reference of a symbol — used by the power-port Style
    /// dropdown to pick a new variant (Bar / Arrow / Wave / Circle / GND …).
    UpdateSymbolLibId {
        symbol_id: Uuid,
        lib_id: String,
        /// The embedded body the symbol draws with after the change
        /// (`Symbol::body_ref`); the caller registers it in the same batch.
        body_ref: String,
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
    /// An empty `value` keeps the field with an empty value; deleting it is
    /// [`Command::RemoveSymbolField`].
    SetSymbolField {
        symbol_id: Uuid,
        key: String,
        value: String,
    },
    /// Delete a custom parameter/field from a symbol. A key the symbol does
    /// not carry, or one of the reserved `reference` / `value` / `footprint`,
    /// changes nothing.
    RemoveSymbolField {
        symbol_id: Uuid,
        key: String,
    },
    /// [`Command::PlaceWireSegment`] that never folds the wire into a
    /// neighbour. Paste places a copied wire with it: the copy keeps the
    /// splits it was cut with (a Break Wire cut, a label or no-connect on the
    /// joint), and the anchors may not be placed yet.
    PlaceWireSegmentAsIs {
        wire: Wire,
    },
    /// Split one wire in two at `at`, which must lie strictly inside it; the
    /// first half keeps the wire's id, the second takes `new_wire`, minted by
    /// the caller (an id already in the sheet is refused). A point
    /// off the wire, or at one of its ends, changes nothing; a spot holding a
    /// pin, bus end, bus entry or another wire's end is refused
    /// (`EngineError::BreakWireBlocked`), since the new end would join it. Connectivity is
    /// unchanged and no merge pass runs, so the halves stay apart.
    BreakWire {
        wire: Uuid,
        at: signex_types::schematic::Point,
        new_wire: Uuid,
    },
    /// Move one end of `wire` (its start when `start`) to `to`, as one undo
    /// step: the grip drag of a selected wire. A horizontal or vertical wire
    /// keeps its line, so `to` is taken along it. With `keep_wires` the wires
    /// attached at that end follow it (see `transform::rubber_band`); the end
    /// leaves a pin or wire it sat on. A move onto the wire's other end
    /// changes nothing.
    MoveWireEnd {
        wire: Uuid,
        start: bool,
        to: signex_types::schematic::Point,
        keep_wires: bool,
    },
    /// Put one end of the wire or bus `run` (its start when `start`) exactly
    /// at `to`, the other end kept: a typed Start or End. Unlike the grip, a
    /// straight run may turn and nothing attached follows. A wire's
    /// junctions are reconciled and collinear wires folded as when a wire
    /// is placed. A `to` on the run's other end changes nothing.
    SetRunEnd {
        run: SelectedItem,
        start: bool,
        to: signex_types::schematic::Point,
    },
    /// Set the justification, turn or text size of one text note; `None`
    /// keeps that property. One undo step, none when nothing changes.
    UpdateTextNoteProps {
        note_id: Uuid,
        props: TextNoteProps,
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
    /// Embed `lib_symbol` in the sheet's `lib_symbols` as its own version,
    /// keyed by `LibSymbol::body_ref` (`"<id>#<hash>"`). Another version of
    /// the same library symbol stays beside it, so old instances keep their
    /// drawing. A no-op when this exact version is already embedded; a
    /// different body under the same ref is
    /// [`EngineError::LibSymbolBodyCollision`]. Run in the same batch as the
    /// `PlaceSymbol` that names the ref so one undo removes both.
    RegisterLibSymbol {
        lib_symbol: LibSymbol,
    },
    /// Move the placed instances of `lib_symbol.id` to the version
    /// `lib_symbol` ("Update from library", #912, #2209): every instance on
    /// the sheet, or only the parts of `only`'s symbols. All units of a part
    /// move together. The versions they leave are dropped once nothing draws
    /// with them. Refused with
    /// [`EngineError::LibSymbolPinsConnected`] when a pin that has a wire
    /// or no-connect on it would vanish or move off that connection;
    /// nothing is mutated then. One undo step.
    ReplaceLibSymbol {
        lib_symbol: LibSymbol,
        /// `(placed symbol uuid, link stamp)` stamps applied in the same
        /// undo step: the pinned version (informational, the content is the
        /// truth, #950) and the row values the part last agreed with, which
        /// move together (#2004).
        library_versions: Vec<(uuid::Uuid, signex_types::schematic::LinkStamp)>,
        /// `Some`: move only the parts these symbols belong to; `None`: all
        /// instances of the library symbol on the sheet.
        only: Option<Vec<uuid::Uuid>>,
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
        directive: NoErcDirective,
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
    /// `renames` run first, on the one symbol they name: its auto-generated
    /// pin named `from` takes the name `to` in place, so its wires stay on
    /// it (#1276); a symbol that already has a `to` pin keeps both pins as
    /// they are, and a hand-placed pin is never renamed.
    ReconcileChildSheetPins {
        child_filename: String,
        ports: Vec<SheetPort>,
        renames: Option<SymbolEntryRenames>,
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
    /// Replace one placed sheet symbol's per-placement entries (its manual
    /// sheet numbers, #2247), by `sheet_id`. Undoable like any other edit.
    SetChildSheetInstances {
        sheet_id: Uuid,
        instances: Vec<signex_types::schematic::SheetInstance>,
    },
    /// Set one placed sheet symbol's `(width, height)` in mm, by `sheet_id`
    /// (#2418). The top-left corner stays; entries on the right and bottom
    /// edges travel with them. Refused whole on a too-small edge, an entry
    /// left past its edge, or a moving entry that is connected
    /// (`exec/child_sheet_size.rs`). Undoable like any other edit.
    ResizeChildSheet {
        sheet_id: Uuid,
        size: (f64, f64),
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
    /// Set the sheet's paper colour (Document Options, #2034); `None` follows
    /// the theme's paper colour.
    SetSheetColor {
        color: Option<signex_types::theme::Color>,
    },
    /// Set the corner the sheet's shown coordinates count from (Document
    /// Options, #2034). Display only: no stored coordinate moves.
    SetPageOrigin {
        origin: signex_types::schematic::PageOrigin,
    },
    /// Set one title-block field of the sheet (`title`, `rev`, `date`,
    /// `company`, `drawn_by`, …; the Properties panel's Title Block section
    /// is the producer). Stored in `SchematicSheet.title_block`; an empty
    /// value removes the key. Undoable like any other edit (#1080).
    SetTitleBlockField {
        key: String,
        value: String,
    },
    /// Set or remove one sheet variable (#1987): `value` is the source text
    /// (a literal or an `=` expression), saved as typed; `None` removes the
    /// variable. Stored in `SchematicSheet::variables`.
    SetSheetVariable {
        name: String,
        value: Option<String>,
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
    /// Replace the sheet's net colour overrides (wire and junction uuid to
    /// colour) with `colors`, as one undo step. One user action is one
    /// command, a "clear all" on this sheet included (an empty map). Unchanged
    /// when `colors` equals the stored map. Entries of elements that are not
    /// in the sheet are kept (a deleted wire's colour returns with the wire on
    /// undo) and dropped when the sheet is encoded.
    SetNetColors {
        colors: std::collections::HashMap<Uuid, signex_types::theme::Color>,
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
    /// Place a Blanket, Compile Mask or Differential Pair directive (#2420).
    PlaceDirective {
        directive: signex_types::schematic::SchDirective,
    },
    /// Replace the directive with `directive`'s uuid by `directive`: its
    /// region and parameters. Refused when no directive of that uuid and kind
    /// exists; unchanged when equal. Consecutive edits of one directive share
    /// one undo step, like typing a field (`Engine::end_field_edit_run`).
    UpdateDirective {
        directive: signex_types::schematic::SchDirective,
    },
    /// Set the ERC rule ids the No-ERC directive `id` silences (#1699); an
    /// empty list silences every rule. Stored sorted without repeats (`*`
    /// stores as empty). Refused when no such directive exists; unchanged
    /// when the stored list is already that.
    SetNoErcRules {
        id: Uuid,
        rules: Vec<String>,
    },
    /// Put the reference and value fields of the parts `symbol_ids` where the
    /// autoplacer puts them, as one undo step (#1667). Asked for by name, so a
    /// field the user moved is placed too and the part's fields count as
    /// autoplaced again. Unchanged when no field moves; an id that names no
    /// part is ignored.
    AutoplaceFields {
        symbol_ids: Vec<Uuid>,
    },
    /// Place a signal harness connector (#1280), with its entries.
    PlaceHarnessConnector {
        connector: signex_types::schematic::HarnessConnector,
    },
    /// Replace the connector `uuid` by `connector` (type, place, size,
    /// entries). Refused when no such connector exists; unchanged when equal.
    /// Consecutive updates of one connector share one undo step, like
    /// [`Command::UpdateDirective`].
    UpdateHarnessConnector {
        uuid: Uuid,
        connector: signex_types::schematic::HarnessConnector,
    },
    /// Insert `entry` into the connector `connector_id` at `index` (clamped
    /// to its entry count).
    PlaceHarnessEntry {
        connector_id: Uuid,
        entry: signex_types::schematic::HarnessEntry,
        index: usize,
    },
    /// Place one harness wire segment.
    PlaceHarnessWire {
        wire: signex_types::schematic::HarnessWire,
    },
    /// Make the sheet entry `sheet_pin_id` harness-typed (`Some`) or a plain
    /// signal entry again (`None`).
    SetHarnessEntryType {
        sheet_pin_id: Uuid,
        harness_type: Option<String>,
    },
    /// Group objects of this sheet under a name and tags (mockup row 158). Refused
    /// with a [`crate::UnionRefusal`] when the name is blank, a member is not
    /// on the sheet or already in another union, or nothing is left. One
    /// undo step.
    CreateUnion {
        union: signex_types::schematic::SchUnion,
    },
    /// Rename, retag, lock or unlock the union `uuid`. Renames and tag edits
    /// of one union are one typing run; a lock change is a step of its own.
    UpdateUnion {
        uuid: Uuid,
        name: String,
        tags: Vec<String>,
        locked: bool,
    },
    /// Replace the members of the union `uuid` (Add / Remove selection). An
    /// empty list breaks it; refused on a locked union.
    SetUnionMembers {
        uuid: Uuid,
        members: Vec<Uuid>,
    },
    /// Remove the union `uuid`; its objects stay. Refused when locked.
    BreakUnion {
        uuid: Uuid,
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

    /// Whether this command acts on a selection, the shape lock pruning
    /// (`Engine::without_locked`) filters item by item.
    #[must_use]
    pub fn is_selection_command(&self) -> bool {
        matches!(
            self,
            Self::MoveSelection { .. }
                | Self::MoveEach { .. }
                | Self::RotateSelection { .. }
                | Self::MirrorSelection { .. }
                | Self::TransformSelection { .. }
                | Self::DeleteSelection { .. }
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
            Command::SetSymbolFlag { .. } => CommandKind::SetSymbolFlag,
            Command::SetWireWidth { .. } => CommandKind::SetWireWidth,
            Command::SetWireColor { .. } => CommandKind::SetWireColor,
            Command::SetSymbolBodyStyle { .. } => CommandKind::SetSymbolBodyStyle,
            Command::UpdateSymbolTextSize { .. } => CommandKind::UpdateSymbolTextSize,
            Command::UpdateSymbolLibId { .. } => CommandKind::UpdateSymbolLibId,
            Command::UpdateSymbolFootprint { .. } => CommandKind::UpdateSymbolFootprint,
            Command::SetSymbolFootprintBinding { .. } => CommandKind::SetSymbolFootprintBinding,
            Command::UpdateSymbolFields { .. } => CommandKind::UpdateSymbolFields,
            Command::SetSymbolField { .. } | Command::RemoveSymbolField { .. } => {
                CommandKind::UpdateSymbolFields
            }
            Command::PlaceWireSegment { .. } => CommandKind::PlaceWireSegment,
            Command::BreakWire { .. } => CommandKind::BreakWire,
            Command::MoveWireEnd { .. } => CommandKind::MoveWireEnd,
            Command::SetRunEnd { .. } => CommandKind::SetRunEnd,
            Command::UpdateTextNoteProps { .. } => CommandKind::UpdateTextNoteProps,
            Command::PlaceWireSegmentAsIs { .. } => CommandKind::PlaceWireSegmentAsIs,
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
            Command::SetChildSheetInstances { .. } => CommandKind::SetChildSheetInstances,
            Command::ResizeChildSheet { .. } => CommandKind::ResizeChildSheet,
            Command::SetPaperSize { .. } => CommandKind::SetPaperSize,
            Command::SetSheetFrame { .. } => CommandKind::SetSheetFrame,
            Command::SetSheetColor { .. } => CommandKind::SetSheetColor,
            Command::SetPageOrigin { .. } => CommandKind::SetPageOrigin,
            Command::SetTitleBlockField { .. } => CommandKind::SetTitleBlockField,
            Command::SetSheetVariable { .. } => CommandKind::SetSheetVariable,
            Command::SetTextFontFamily { .. } => CommandKind::SetTextFontFamily,
            Command::SetSymbolDesignatorIn { .. } => CommandKind::SetSymbolDesignatorIn,
            Command::SetDocumentFontFamily { .. } => CommandKind::SetDocumentFontFamily,
            Command::SetNetColors { .. } => CommandKind::SetNetColors,
            Command::ArrangeDrawings { .. } => CommandKind::ArrangeDrawings,
            Command::SetDrawingPlacement { .. } => CommandKind::SetDrawingPlacement,
            Command::PlaceDirective { .. } => CommandKind::PlaceDirective,
            Command::UpdateDirective { .. } => CommandKind::UpdateDirective,
            Command::SetNoErcRules { .. } => CommandKind::SetNoErcRules,
            Command::AutoplaceFields { .. } => CommandKind::AutoplaceFields,
            Command::PlaceHarnessConnector { .. } => CommandKind::PlaceHarnessConnector,
            Command::UpdateHarnessConnector { .. } => CommandKind::UpdateHarnessConnector,
            Command::PlaceHarnessEntry { .. } => CommandKind::PlaceHarnessEntry,
            Command::PlaceHarnessWire { .. } => CommandKind::PlaceHarnessWire,
            Command::SetHarnessEntryType { .. } => CommandKind::SetHarnessEntryType,
            Command::CreateUnion { .. } => CommandKind::CreateUnion,
            Command::UpdateUnion { .. } => CommandKind::UpdateUnion,
            Command::SetUnionMembers { .. } => CommandKind::SetUnionMembers,
            Command::BreakUnion { .. } => CommandKind::BreakUnion,
        }
    }
}
