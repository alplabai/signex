//! What the primitive panels show, copied from the board by the app
//! (`app/handlers/pcb_primitive`) so the views stay pure.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use pcb_geom::ItemRef;
use signex_types::pcb::PadShape;
use signex_types::pcb_layer::{CopperLayer, PcbLayer};

/// The board and the selected items a panel was built for. Every edit
/// message carries it, so a selection change in between cannot send the
/// edit to other items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveTarget {
    pub board: PathBuf,
    pub items: Vec<ItemRef>,
}

/// A typed field of a primitive panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveField {
    /// The location x: a track selection's corner, a via's or pad's centre.
    X,
    Y,
    Width,
    StartX,
    StartY,
    EndX,
    EndY,
    Diameter,
    HoleSize,
    Designator,
    Rotation,
    SizeX,
    SizeY,
}

impl PrimitiveField {
    /// A field typed as text, not as a number.
    pub fn is_text(self) -> bool {
        self == Self::Designator
    }
}

/// A net in the Net dropdown; 0 is No Net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetChoice {
    pub number: u32,
    pub label: String,
}

impl fmt::Display for NetChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// A copper layer in the Layer dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerChoice {
    pub layer: CopperLayer,
    pub label: String,
}

impl fmt::Display for LayerChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// A pad shape in the Shape dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeChoice {
    pub shape: PadShape,
    pub label: String,
}

impl fmt::Display for ShapeChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// Whether the selection can be edited, and if not why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editability {
    Editable,
    /// A pad's footprint is locked.
    Locked,
    /// The board opened read-only (a newer format).
    ReadOnly,
}

/// The Net Information section. A value the selection does not share is
/// `None` and shows blank.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NetInfo {
    /// The shared net; `Some(0)` is no net.
    pub net: Option<u32>,
    pub name: Option<String>,
    pub class: Option<String>,
    /// The routed length of the selected tracks; `None` where it does not
    /// apply (a via or pad).
    pub selected_length_mm: Option<f64>,
    /// The routed length of the shared net.
    pub total_length_mm: Option<f64>,
    pub tracks: usize,
    pub vias: usize,
    pub pads: usize,
}

/// What every primitive panel has: its target, net and panel state.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveCommon {
    pub target: PrimitiveTarget,
    pub editability: Editability,
    pub net_info: NetInfo,
    /// No Net first, then the board's nets by number.
    pub nets: Vec<NetChoice>,
    pub net: Option<NetChoice>,
    /// Text typed into a field and not applied yet.
    pub drafts: BTreeMap<PrimitiveField, String>,
    pub show_more: bool,
    /// The header's search text: only rows whose label contains it show.
    pub search: String,
    /// The pick button is waiting for a click on the board.
    pub picking_net: bool,
}

impl PrimitiveCommon {
    pub fn is_editable(&self) -> bool {
        self.editability == Editability::Editable
    }

    /// Whether a row labelled `label` passes the search.
    pub fn shows(&self, label: &str) -> bool {
        let query = self.search.trim();
        query.is_empty() || label.to_lowercase().contains(&query.to_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackFacts {
    pub common: PrimitiveCommon,
    /// The lower-left corner of the selection's ends.
    pub location: Option<(f64, f64)>,
    pub layer: Option<LayerChoice>,
    /// The board's copper layers, top to bottom.
    pub layers: Vec<LayerChoice>,
    pub width: Option<f64>,
    pub start: (Option<f64>, Option<f64>),
    pub end: (Option<f64>, Option<f64>),
    pub length_mm: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViaFacts {
    pub common: PrimitiveCommon,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub diameter: Option<f64>,
    pub hole: Option<f64>,
    /// Its span name, "Thru 1:4"; `None` when the vias differ.
    pub span: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PadFacts {
    pub common: PrimitiveCommon,
    pub designator: Option<String>,
    /// The pad's layer as shown, with the layer its chip is coloured by.
    pub layer: Option<(String, Option<PcbLayer>)>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub rotation: Option<f64>,
    pub size_x: Option<f64>,
    pub size_y: Option<f64>,
    pub hole: Option<f64>,
    /// Every selected pad has a hole.
    pub has_hole: bool,
    pub shape: Option<ShapeChoice>,
    pub shapes: Vec<ShapeChoice>,
}

/// The selection the Properties panel shows as a primitive.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbPrimitiveFacts {
    Track(TrackFacts),
    Via(ViaFacts),
    Pad(PadFacts),
}

impl PcbPrimitiveFacts {
    pub fn common(&self) -> &PrimitiveCommon {
        match self {
            Self::Track(f) => &f.common,
            Self::Via(f) => &f.common,
            Self::Pad(f) => &f.common,
        }
    }
}

/// The value `values` share, or `None` when they differ or there are none.
pub fn shared<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}
