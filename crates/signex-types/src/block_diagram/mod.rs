//! The block diagram document (`.snxblk`, mockup row 171, #2796): the
//! system view of a design as blocks, their ports, the signal-flow
//! connections between them with a protocol label, groups, and the link
//! from a block to the schematic sheet it stands for.
//!
//! Plain data, UI-free. Lengths are `f64` mm in memory and integer nm on
//! disk (`file`). Edits go through the methods in `edit`, which keep every
//! reference inside the document resolvable; `fault` is the one check of
//! that, run before every write and after every read. The file layout is in
//! `docs/architecture/snxblk-format.md`.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};
use uuid::Uuid;

use crate::document_id::DocumentId;
use crate::schematic::Point;
use crate::theme::Color;

mod edit;
mod fault;
mod file;
mod geometry;
mod writer;

#[cfg(test)]
mod edit_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod geometry_tests;

pub use edit::{BlockDiagramError, BlockFields, BlockPatch, RemovedBlock};
pub use fault::DiagramFault;
pub use file::{
    BlockDiagramFileError, ReadBlockDiagram, SNXBLK_FORMAT_V1, SNXBLK_VERSION, parse_block_diagram,
    read_block_diagram, write_block_diagram,
};
pub use geometry::{DiagramBox, GROUP_MARGIN_MM, PLACEMENT_GAP_MM, distance_to_segment};
pub use writer::encode_block_diagram;

/// The file extension of a block diagram, without the dot.
pub const BLOCK_DIAGRAM_EXTENSION: &str = "snxblk";

/// Width of a block made without one, mm.
pub const DEFAULT_BLOCK_WIDTH_MM: f64 = 40.0;

/// Height of a block made without one, mm.
pub const DEFAULT_BLOCK_HEIGHT_MM: f64 = 25.0;

/// One block diagram document: everything a `.snxblk` file holds.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockDiagram {
    /// The document's persistent id (`docs/DOCUMENT_IDS.md`).
    pub id: DocumentId,
    /// The groups, in the order they were made.
    pub groups: Vec<BlockGroup>,
    /// The blocks, in drawing order: a later block draws over an earlier one.
    pub blocks: Vec<Block>,
    /// The connections, in the order they were made.
    pub connections: Vec<Connection>,
}

impl BlockDiagram {
    /// An empty diagram with a fresh document id.
    pub fn new() -> Self {
        Self::with_id(DocumentId::new())
    }

    /// An empty diagram that carries `id`.
    pub fn with_id(id: DocumentId) -> Self {
        Self {
            id,
            groups: Vec::new(),
            blocks: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// The block with id `id`.
    pub fn block(&self, id: Uuid) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// The connection with id `id`.
    pub fn connection(&self, id: Uuid) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// The group with id `id`.
    pub fn group(&self, id: Uuid) -> Option<&BlockGroup> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// The block linked to the sheet `sheet`, linked or marked unlinked.
    pub fn block_for_sheet(&self, sheet: DocumentId) -> Option<&Block> {
        self.blocks
            .iter()
            .find(|b| b.sheet.as_ref().is_some_and(|link| link.sheet == sheet))
    }

    /// Every connection that ends on block `block`, at any of its ports.
    pub fn connections_of(&self, block: Uuid) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |c| c.touches(block))
    }
}

impl Default for BlockDiagram {
    fn default() -> Self {
        Self::new()
    }
}

/// One functional block: a box on the canvas with its properties
/// (function, rails, key specs) and its ports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    /// The block's id, unique in the document.
    pub id: Uuid,
    /// The name drawn in the block; never blank.
    pub name: String,
    /// What the block does, one line ("3 A buck, 570 kHz"); may be empty.
    pub function: String,
    /// A free note shown under the block; may be empty.
    pub note: String,
    /// The outline the block is drawn with.
    pub shape: BlockShape,
    /// The top-left corner, mm.
    pub position: Point,
    /// Width, mm; finite and above zero.
    pub width_mm: f64,
    /// Height, mm; finite and above zero.
    pub height_mm: f64,
    /// The person's colour for the block; `None` draws it in the theme's.
    pub color: Option<Color>,
    /// The group the block belongs to.
    pub group: Option<Uuid>,
    /// The supply rails the block uses or makes, by net name.
    pub rails: Vec<String>,
    /// Key specs, name to value ("Vout" to "3.30 V"), sorted by name.
    pub specs: BTreeMap<String, String>,
    /// The block's ports, in the order they were made.
    pub ports: Vec<BlockPort>,
    /// The schematic sheet the block stands for; `None` for a block drawn
    /// by hand that no sheet backs.
    pub sheet: Option<SheetLink>,
}

impl Block {
    /// The port with id `id` on this block.
    pub fn port(&self, id: Uuid) -> Option<&BlockPort> {
        self.ports.iter().find(|p| p.id == id)
    }
}

/// The outline a block is drawn with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockShape {
    /// A plain rectangle.
    #[default]
    Rectangle,
    /// A rectangle with rounded corners.
    RoundedRectangle,
    /// An ellipse inside the block's box.
    Ellipse,
    /// A hexagon with its points on the left and right sides of the box.
    Hexagon,
}

crate::token_enum! {
    pub(crate) fn shape_token / shape_from_token for BlockShape, error TokenError {
        Rectangle => "rectangle",
        RoundedRectangle => "rounded_rectangle",
        Ellipse => "ellipse",
        Hexagon => "hexagon",
    }
}

/// One port on a block's edge, where connections end.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockPort {
    /// The port's id, unique in the document.
    pub id: Uuid,
    /// The port's name, usually the sheet port or net it stands for; never
    /// blank.
    pub name: String,
    /// The edge of the block the port sits on.
    pub side: PortSide,
    /// Distance along that edge from its top or left end, mm; finite and
    /// not negative.
    pub offset_mm: f64,
    /// Which way signal or power flows through the port.
    pub direction: PortDirection,
}

/// The edge of a block a port sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortSide {
    /// The left edge; the offset runs down from its top end.
    #[default]
    Left,
    /// The right edge; the offset runs down from its top end.
    Right,
    /// The top edge; the offset runs right from its left end.
    Top,
    /// The bottom edge; the offset runs right from its left end.
    Bottom,
}

crate::token_enum! {
    pub(crate) fn side_token / side_from_token for PortSide, error TokenError {
        Left => "left",
        Right => "right",
        Top => "top",
        Bottom => "bottom",
    }
}

/// Which way signal or power flows through a port, seen from its block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortDirection {
    /// Into the block.
    Input,
    /// Out of the block.
    Output,
    /// Both ways, or not known.
    #[default]
    Bidirectional,
}

crate::token_enum! {
    pub(crate) fn direction_token / direction_from_token for PortDirection, error TokenError {
        Input => "input",
        Output => "output",
        Bidirectional => "bidirectional",
    }
}

/// A signal-flow connection from one block to another, labelled with its
/// interface.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Connection {
    /// The connection's id, unique in the document.
    pub id: Uuid,
    /// Where the flow starts.
    pub from: Endpoint,
    /// Where the flow ends; never the same endpoint as `from`.
    pub to: Endpoint,
    /// The protocol or kind of signal the connection carries.
    pub interface: Interface,
    /// The text drawn on the connection ("SDA, SCL"); may be empty.
    pub label: String,
    /// The net, bus or harness a generated connection was made from; `None`
    /// for a connection drawn by hand.
    pub derived_from: Option<String>,
}

impl Connection {
    /// Whether either end of the connection is on block `block`.
    pub fn touches(&self, block: Uuid) -> bool {
        self.from.block == block || self.to.block == block
    }
}

/// One end of a connection: a block, and the port on it when the
/// connection ends on one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Endpoint {
    /// The block the connection ends on.
    pub block: Uuid,
    /// The port on that block; `None` ends on the block's edge.
    pub port: Option<Uuid>,
}

/// The protocol or kind of signal a connection carries (the interface
/// annotations of the ROADMAP's Block Diagram band). A name this build does
/// not know is kept as [`Interface::Other`] and written back unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Interface {
    /// A plain signal with no protocol.
    #[default]
    Signal,
    /// A bus of several signals.
    Bus,
    /// A supply rail.
    Power,
    /// An analog signal.
    Analog,
    /// An SPI link.
    Spi,
    /// An I2C link.
    I2c,
    /// A UART link.
    Uart,
    /// A USB link.
    Usb,
    /// An LVDS link.
    Lvds,
    /// Any other interface, by its name ("CAN", "MIPI CSI"); never blank.
    Other(String),
}

/// `(variant, token)`, for every interface but [`Interface::Other`].
const INTERFACE_TOKENS: [(Interface, &str); 9] = [
    (Interface::Signal, "signal"),
    (Interface::Bus, "bus"),
    (Interface::Power, "power"),
    (Interface::Analog, "analog"),
    (Interface::Spi, "spi"),
    (Interface::I2c, "i2c"),
    (Interface::Uart, "uart"),
    (Interface::Usb, "usb"),
    (Interface::Lvds, "lvds"),
];

impl Interface {
    /// The text the file and the commands use: a fixed token, or the name
    /// of an [`Interface::Other`] as it was given.
    pub fn token(&self) -> &str {
        if let Self::Other(name) = self {
            return name;
        }
        INTERFACE_TOKENS
            .iter()
            .find(|(interface, _)| interface == self)
            .map_or("signal", |(_, token)| *token)
    }

    /// The interface `text` names. A known token matches ignoring case and
    /// surrounding spaces; any other text is an [`Interface::Other`] with
    /// the spaces trimmed.
    pub fn from_token(text: &str) -> Self {
        let text = text.trim();
        INTERFACE_TOKENS
            .iter()
            .find(|(_, token)| token.eq_ignore_ascii_case(text))
            .map_or_else(|| Self::Other(text.to_string()), |(i, _)| i.clone())
    }
}

impl Serialize for Interface {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.token())
    }
}

/// A named group of blocks, drawn as a frame around them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockGroup {
    /// The group's id, unique in the document.
    pub id: Uuid,
    /// The name drawn on the frame; never blank.
    pub name: String,
    /// The person's colour for the frame; `None` draws it in the theme's.
    pub color: Option<Color>,
}

/// The link from a block to the schematic sheet it stands for (#2968).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SheetLink {
    /// The sheet's document id, which a rename or move keeps.
    pub sheet: DocumentId,
    /// The sheet's path relative to the project folder, `/`-separated, as
    /// last seen; never blank.
    pub path: String,
    /// Whether the sheet is still in the project.
    pub state: LinkState,
}

/// Whether a linked sheet is still in the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// The sheet is in the project.
    #[default]
    Linked,
    /// The sheet was removed; the block stays, marked, until the person
    /// deletes or relinks it.
    Unlinked,
}

crate::token_enum! {
    pub(crate) fn link_state_token / link_state_from_token for LinkState, error TokenError {
        Linked => "linked",
        Unlinked => "unlinked",
    }
}

/// A file token that names no value of its kind.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("`{got}` is not a {kind} this Signex reads")]
    UnknownEnumToken { kind: &'static str, got: String },
}
