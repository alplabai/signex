use signex_types::schematic::{
    Bus, BusEntry, Junction, Label, NoConnect, SchematicSheet, SelectedItem, Symbol, TextNote,
    Wire,
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
pub enum CommandKind {
    ReplaceDocument,
    MoveSelection,
    RotateSelection,
    MirrorSelection,
    DeleteSelection,
    UpdateText,
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