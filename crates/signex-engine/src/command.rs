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
            Command::PlaceWireSegment { .. } => CommandKind::PlaceWireSegment,
            Command::PlaceBus { .. } => CommandKind::PlaceBus,
            Command::PlaceLabel { .. } => CommandKind::PlaceLabel,
            Command::PlaceSymbol { .. } => CommandKind::PlaceSymbol,
            Command::PlaceJunction { .. } => CommandKind::PlaceJunction,
            Command::PlaceNoConnect { .. } => CommandKind::PlaceNoConnect,
            Command::PlaceBusEntry { .. } => CommandKind::PlaceBusEntry,
            Command::PlaceTextNote { .. } => CommandKind::PlaceTextNote,
        }
    }
}
