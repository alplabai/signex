use signex_types::schematic::{
    Bus, BusEntry, HAlign, Junction, Label, NoConnect, SchematicSheet, SelectedItem, Symbol,
    TextNote, Wire,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirrorAxis {
    Horizontal,
    Vertical,
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
    RotateSelection,
    MirrorSelection,
    DeleteSelection,
    UpdateText,
    UpdateLabelProps,
    SetSymbolRotation,
    UpdateSymbolTextSize,
    UpdateSymbolLibId,
    UpdateSymbolFootprint,
    UpdateSymbolFields,
    PlaceWireSegment,
    PlaceBus,
    PlaceLabel,
    PlaceSymbol,
    PlaceJunction,
    PlaceNoConnect,
    PlaceBusEntry,
    PlaceTextNote,
    PlaceSchDrawing,
    UpdateSchDrawing,
    AnnotateAll,
    MoveSymbolAbsolute,
    ReorderObjects,
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
    },
    RotateSelection {
        items: Vec<SelectedItem>,
        angle_degrees: f64,
    },
    MirrorSelection {
        items: Vec<SelectedItem>,
        axis: MirrorAxis,
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
    PlaceJunction {
        junction: Junction,
    },
    PlaceNoConnect {
        no_connect: NoConnect,
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
    /// Auto-annotate every symbol whose reference ends in `?` with a unique
    /// sequential designator per prefix (R?, C?, U?, …). Applied in a
    /// deterministic order so re-running produces the same layout.
    AnnotateAll {
        mode: AnnotateMode,
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
    /// of each type vector. KiCad schematic has no explicit z-order, so
    /// render order is file order; this command mutates that.
    ReorderObjects {
        items: Vec<SelectedItem>,
        direction: ReorderDirection,
    },
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
    pub fn kind(&self) -> CommandKind {
        match self {
            Command::ReplaceDocument { .. } => CommandKind::ReplaceDocument,
            Command::MoveSelection { .. } => CommandKind::MoveSelection,
            Command::RotateSelection { .. } => CommandKind::RotateSelection,
            Command::MirrorSelection { .. } => CommandKind::MirrorSelection,
            Command::DeleteSelection { .. } => CommandKind::DeleteSelection,
            Command::UpdateText { .. } => CommandKind::UpdateText,
            Command::UpdateLabelProps { .. } => CommandKind::UpdateLabelProps,
            Command::SetSymbolRotation { .. } => CommandKind::SetSymbolRotation,
            Command::UpdateSymbolTextSize { .. } => CommandKind::UpdateSymbolTextSize,
            Command::UpdateSymbolLibId { .. } => CommandKind::UpdateSymbolLibId,
            Command::UpdateSymbolFootprint { .. } => CommandKind::UpdateSymbolFootprint,
            Command::UpdateSymbolFields { .. } => CommandKind::UpdateSymbolFields,
            Command::SetSymbolField { .. } => CommandKind::UpdateSymbolFields,
            Command::PlaceWireSegment { .. } => CommandKind::PlaceWireSegment,
            Command::PlaceBus { .. } => CommandKind::PlaceBus,
            Command::PlaceLabel { .. } => CommandKind::PlaceLabel,
            Command::PlaceSymbol { .. } => CommandKind::PlaceSymbol,
            Command::PlaceJunction { .. } => CommandKind::PlaceJunction,
            Command::PlaceNoConnect { .. } => CommandKind::PlaceNoConnect,
            Command::PlaceBusEntry { .. } => CommandKind::PlaceBusEntry,
            Command::PlaceTextNote { .. } => CommandKind::PlaceTextNote,
            Command::PlaceSchDrawing { .. } => CommandKind::PlaceSchDrawing,
            Command::UpdateSchDrawing { .. } => CommandKind::UpdateSchDrawing,
            Command::AnnotateAll { .. } => CommandKind::AnnotateAll,
            Command::MoveSymbolAbsolute { .. } => CommandKind::MoveSymbolAbsolute,
            Command::ReorderObjects { .. } => CommandKind::ReorderObjects,
        }
    }
}
