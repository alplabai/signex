use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::format::{BoardUnknown, UnknownColumns, UnknownFields, parse_pcb_layer};
use crate::pad_thermal::PadThermal;
use crate::part::FootprintSourceRef;
use crate::pcb_layer::{CopperLayer, PcbLayer};
pub use crate::pcb_stack::{LayerSize, LayerSizes, LayerStack};
use crate::property::PcbProperty;
use crate::schematic::InstancePath;

pub use crate::pcb_component::{ComponentAttributes, ComponentType};
pub use crate::pcb_route_attrs::{MaskExpansion, TrackAttributes, ViaAttributes, ViaMaskSide};
pub use crate::schematic::Point;
pub use room::{Room, RoomBounds, RoomLayer, RoomOutline, RoomShape, RoomSource};

pub mod room;

// ---------------------------------------------------------------------------
// PCB domain constants
// ---------------------------------------------------------------------------

/// Standard PCB board thickness in mm (1.6 mm = 63 mil).
pub const PCB_DEFAULT_THICKNESS_MM: f64 = 1.6;

/// Default PCB grid size in mm (1.27 mm = 50 mil).
pub const PCB_GRID_MM: f64 = 1.27;

/// Default trace / segment width in mm (0.25 mm = ~10 mil).
pub const PCB_DEFAULT_TRACE_WIDTH_MM: f64 = 0.25;

/// Minimum trace width fallback in mm (0.1 mm = ~4 mil).
pub const PCB_TRACK_MIN_MM: f64 = 0.1;

/// Default via finished-hole diameter in mm (0.6 mm = ~24 mil).
pub const PCB_DEFAULT_VIA_DIAMETER_MM: f64 = 0.6;

/// Default via drill diameter in mm (0.3 mm = ~12 mil).
pub const PCB_DEFAULT_VIA_DRILL_MM: f64 = 0.3;

/// Minimum via diameter fallback in mm (0.4 mm = ~16 mil).
pub const PCB_VIA_MIN_DIAMETER_MM: f64 = 0.4;

/// Minimum via drill fallback in mm (0.2 mm = ~8 mil).
pub const PCB_VIA_MIN_DRILL_MM: f64 = 0.2;

/// Default design-rule clearance in mm (0.2 mm = ~8 mil).
pub const PCB_DEFAULT_CLEARANCE_MM: f64 = 0.2;

/// Default copper-zone minimum fill thickness in mm (0.254 mm = 10 mil).
pub const PCB_ZONE_MIN_THICKNESS_MM: f64 = 0.254;

/// Default copper-zone thermal relief gap in mm (0.508 mm = 20 mil).
pub const PCB_THERMAL_GAP_MM: f64 = 0.508;

/// Default copper-zone thermal bridge width in mm (0.254 mm = 10 mil).
pub const PCB_THERMAL_BRIDGE_MM: f64 = 0.254;

/// Default graphic / silkscreen line width in mm (0.1 mm = ~4 mil).
pub const PCB_DEFAULT_GRAPHIC_WIDTH_MM: f64 = 0.1;

/// Default pad size (width and height) in mm (1.0 mm = ~39 mil).
pub const PCB_DEFAULT_PAD_SIZE_MM: f64 = 1.0;

/// Default PCB text / fp_text font size in mm (1.0 mm = ~39 mil).
pub const PCB_DEFAULT_TEXT_SIZE_MM: f64 = 1.0;

/// Stroke thickness for PCB text in mm (0.15 mm = ~6 mil).
pub const PCB_TEXT_THICKNESS_MM: f64 = 0.15;

/// Vertical offset for auto-placed Reference / Value fp_text in mm (2.0 mm = ~79 mil).
pub const PCB_FP_TEXT_OFFSET_MM: f64 = 2.0;

// ---------------------------------------------------------------------------
// Pad / Via enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadType {
    Thru,
    Smd,
    Connect,
    NpThru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadShape {
    Circle,
    Rect,
    Oval,
    RoundRect,
    /// A rectangle with one pair of opposite edges of different lengths,
    /// set by [`Pad::trapezoid_delta`] (#2242).
    Trapezoid,
    /// A polygon of the pad's own, [`Pad::custom_outline`] (#2242).
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViaType {
    Through,
    Blind,
    Micro,
}

// ---------------------------------------------------------------------------
// Layer definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerDef {
    pub id: u8,
    pub name: String,
    pub layer_type: String,
    /// User-editable name shown for a mechanical layer; empty when unset.
    /// Saved in the optional `[stackup.display_names]` table, keyed by layer name.
    #[serde(default)]
    pub display_name: String,
}

impl LayerDef {
    /// The name to show: the display name when set, else the layer name.
    pub fn display_label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.name
        } else {
            &self.display_name
        }
    }
}

// ---------------------------------------------------------------------------
// PCB setup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbSetup {
    #[serde(default)]
    pub grid_size: f64,
    #[serde(default)]
    pub trace_width: f64,
    #[serde(default)]
    pub via_diameter: f64,
    #[serde(default)]
    pub via_drill: f64,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub track_min_width: f64,
    #[serde(default)]
    pub via_min_diameter: f64,
    #[serde(default)]
    pub via_min_drill: f64,
}

// ---------------------------------------------------------------------------
// Net definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetDef {
    pub number: u32,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Drill definition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrillDef {
    pub diameter: f64,
    #[serde(default)]
    pub shape: String,
}

// ---------------------------------------------------------------------------
// Pad net reference
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PadNet {
    pub number: u32,
    pub name: String,
}

// ---------------------------------------------------------------------------
// Pad
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pad {
    pub uuid: Uuid,
    #[serde(default)]
    pub number: String,
    pub pad_type: PadType,
    pub shape: PadShape,
    pub position: Point,
    pub size: Point,
    pub drill: Option<DrillDef>,
    #[serde(default)]
    pub layers: Vec<String>,
    pub net: Option<PadNet>,
    /// RoundRect corner radius as a fraction of the shorter side, in
    /// `0.0..=0.5`; read only for [`PadShape::RoundRect`].
    #[serde(default)]
    pub roundrect_ratio: f64,
    /// The pad's own turn in degrees, counter-clockwise in the footprint's
    /// frame, composed with the footprint's rotation. Saved in the optional
    /// `rotation` pad column (`snxpcb/1.3`).
    #[serde(default)]
    pub rotation: f64,
    /// How much longer, in mm, the pad's +y edge is than its −y edge
    /// (`x`), or its +x edge than its −x edge (`y`); at most one is
    /// non-zero. Read only for [`PadShape::Trapezoid`]; see
    /// [`crate::pcb_pad_shape`]. Saved in the optional `trapezoid_delta`
    /// pad column (`snxpcb/1.4`).
    #[serde(default)]
    pub trapezoid_delta: Point,
    /// The copper outline in the pad's own frame, mm, about
    /// [`Self::position`]: a simple polygon that turns with the pad. Read
    /// only for [`PadShape::Custom`]. Saved in the optional
    /// `custom_outline` pad column (`snxpcb/1.4`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_outline: Vec<Point>,
    /// The net was set by hand and the schematic never changes it.
    /// Saved in the optional `net_locked` pad column.
    #[serde(default)]
    pub net_locked: bool,
    /// Copper sizes that differ by layer (#2401); `None` is [`Self::size`]
    /// on every layer. Saved in the optional `layer_sizes` pad column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_sizes: Option<LayerSizes<Point>>,
    /// The pad's own thermal relief values (#2593), each `None` following
    /// the pour. Saved in the optional `thermal` pad column.
    #[serde(default, skip_serializing_if = "PadThermal::is_rule")]
    pub thermal: PadThermal,
    /// Keys in this pad's `[extras.pads.<uuid>]` table that this build
    /// does not recognise, kept so a save re-emits them.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

// ---------------------------------------------------------------------------
// Footprint graphic
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FpGraphic {
    #[serde(default)]
    pub graphic_type: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    pub mid: Option<Point>,
    #[serde(default)]
    pub radius: f64,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub font_size: f64,
    pub position: Option<Point>,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub fill: String,
}

// ---------------------------------------------------------------------------
// Footprint
// ---------------------------------------------------------------------------

/// The placed schematic part a footprint belongs to: the sheet occurrence it
/// sits on plus the UUIDs of every unit of the part (a multi-unit part is
/// several symbol instances that share a designator). Unlike a designator,
/// neither changes when the schematic is re-annotated. Saved in the optional
/// `sch_link` footprint column. See ADR-0005 section 11.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolLink {
    pub sheet: InstancePath,
    /// Unit symbol UUIDs, sorted and without repeats; build with [`Self::new`].
    pub units: Vec<Uuid>,
}

impl SymbolLink {
    /// A link to the part made of `units` on `sheet`.
    pub fn new(sheet: InstancePath, units: impl IntoIterator<Item = Uuid>) -> Self {
        let mut units: Vec<Uuid> = units.into_iter().collect();
        units.sort_unstable();
        units.dedup();
        Self { sheet, units }
    }

    /// Whether this link names the part made of `units` on `sheet`: the same
    /// sheet path and at least one shared unit. The one place that rule lives.
    pub fn names_part(&self, sheet: &InstancePath, units: &[Uuid]) -> bool {
        self.sheet == *sheet && self.units.iter().any(|u| units.contains(u))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Footprint {
    pub uuid: Uuid,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint_id: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub pads: Vec<Pad>,
    #[serde(default)]
    pub graphics: Vec<FpGraphic>,
    #[serde(default)]
    pub properties: Vec<PcbProperty>,
    /// The schematic symbol this footprint is the board side of; `None` for
    /// a footprint placed by hand or on a board saved before the link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schematic_link: Option<SymbolLink>,
    /// The library footprint this one was built from; `None` for a footprint
    /// drawn on the board or saved before the field. Saved in the optional
    /// `lib_ref` footprint column. See ADR-0005 section 13.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library_ref: Option<FootprintSourceRef>,
    /// Type, height, locks and the other component settings (#2365). Boxed
    /// so a footprint stays small beside the other board items.
    #[serde(default, skip_serializing_if = "ComponentAttributes::is_default")]
    pub component: Box<ComponentAttributes>,
    /// Keys in this footprint's `[extras.footprints.<uuid>]` table that
    /// this build does not recognise, kept so a save re-emits them.
    #[serde(default, skip_serializing_if = "UnknownFields::is_empty")]
    pub unknown: UnknownFields,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

// ---------------------------------------------------------------------------
// Traces / routing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub net: u32,
    /// Move, delete and edits other than the lock itself are refused
    /// (#2407). Saved in the optional `locked` track column.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Mask settings (#2378); the lock is `locked`.
    #[serde(default, skip_serializing_if = "TrackAttributes::is_default")]
    pub attributes: TrackAttributes,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Via {
    pub uuid: Uuid,
    pub position: Point,
    #[serde(default)]
    pub diameter: f64,
    #[serde(default)]
    pub drill: f64,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default)]
    pub net: u32,
    #[serde(default = "default_via_type")]
    pub via_type: ViaType,
    /// Move, delete and edits other than the lock itself are refused
    /// (#2407). Saved in the optional `locked` via column.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// Diameters that differ by layer (#2401); `None` is [`Self::diameter`]
    /// on every layer. Saved in the optional `layer_sizes` via column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_sizes: Option<LayerSizes<f64>>,
    /// Mask and tenting settings (#2378); the lock is `locked`.
    #[serde(default, skip_serializing_if = "ViaAttributes::is_default")]
    pub attributes: ViaAttributes,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

fn default_via_type() -> ViaType {
    ViaType::Through
}

impl Via {
    /// The copper diameter on `layer`: its per-layer size there, else
    /// [`Self::diameter`].
    pub fn diameter_on(&self, layer: CopperLayer) -> f64 {
        self.layer_sizes
            .as_ref()
            .and_then(|sizes| sizes.on(layer))
            .unwrap_or(self.diameter)
    }

    /// The smallest copper diameter on the layers the via passes through:
    /// those of `copper` (the board's copper, top to bottom) between its two
    /// span ends, every one of them when a span end is not copper.
    /// [`Self::diameter`] counts only on a layer its sizes do not cover, as
    /// top/middle/bottom sizes cover every layer.
    pub fn min_diameter(&self, copper: &[CopperLayer]) -> f64 {
        let ends: Option<Vec<CopperLayer>> = self
            .layers
            .iter()
            .map(|name| parse_pcb_layer(name).and_then(PcbLayer::copper))
            .collect();
        let span = ends
            .filter(|ends| !ends.is_empty())
            .and_then(|ends| Some((*ends.iter().min()?, *ends.iter().max()?)));
        copper
            .iter()
            .filter(|&&layer| span.is_none_or(|(from, to)| (from..=to).contains(&layer)))
            .map(|&layer| self.diameter_on(layer))
            .reduce(f64::min)
            .unwrap_or(self.diameter)
    }
}

impl Pad {
    /// The copper size on `layer`: its per-layer size there, else
    /// [`Self::size`].
    pub fn size_on(&self, layer: CopperLayer) -> Point {
        self.layer_sizes
            .as_ref()
            .and_then(|sizes| sizes.on(layer))
            .unwrap_or(self.size)
    }
}

// ---------------------------------------------------------------------------
// Zone
// ---------------------------------------------------------------------------

/// The `fill_type` of solid copper drawn as a Fill or a Solid Region (#2373).
pub const SOLID_FILL: &str = "solid";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub uuid: Uuid,
    #[serde(default)]
    pub net: u32,
    #[serde(default)]
    pub net_name: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub outline: Vec<Point>,
    /// Cut-outs inside `outline` (keep-out islands): each entry is a closed
    /// polygon the zone does not fill (#818). Not written when empty, so a
    /// zone without holes keeps its exact pre-#818 bytes. A hole with fewer
    /// than 3 vertices is malformed and consumers must skip it, as the
    /// renderer skips an outline with `len() < 3`; the parser does not
    /// validate this.
    ///
    /// Holes must be simple rings, disjoint from each other and strictly
    /// inside `outline`, touching neither it nor each other: the GPU
    /// triangulation and the CPU even-odd fill only agree for such holes. A
    /// cut-out that reaches the outline is a notch, and belongs in `outline`
    /// itself. Both render paths draw only [`Zone::usable_holes`]; each hole
    /// that breaks the rule is drawn as copper (the valid ones still cut
    /// out), and the app reports it when the board is opened (#897).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub holes: Vec<Vec<Point>>,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub fill_type: String,
    #[serde(default)]
    pub thermal_relief: bool,
    #[serde(default)]
    pub thermal_gap: f64,
    #[serde(default)]
    pub thermal_width: f64,
    #[serde(default)]
    pub clearance: f64,
    #[serde(default)]
    pub min_thickness: f64,
    /// A pour's islands smaller than this, in mm², are dropped from its
    /// fill; 0 keeps every island. Not written when 0.
    #[serde(default, skip_serializing_if = "crate::zone_pour::is_unset_area")]
    pub min_island_area: f64,
    /// A pour drops the islands of its fill that touch no copper of its
    /// net. Not written when off.
    #[serde(default, skip_serializing_if = "crate::zone_pour::is_off")]
    pub remove_dead_islands: bool,
    /// A shelved pour keeps its outline and settings but holds no copper:
    /// it is not filled and clears nothing (#2523). Not written when off.
    #[serde(default, skip_serializing_if = "crate::zone_pour::is_off")]
    pub shelved: bool,
    /// An unfilled pour keeps its outline and settings but holds no copper
    /// and clears nothing until the next Repour, which clears the flag
    /// (#2506). Unlike a shelved pour it counts as not filled. Not written
    /// when off.
    #[serde(default, skip_serializing_if = "crate::zone_pour::is_off")]
    pub unfilled: bool,
    /// Keys in this zone's `[[zones]]` table that this build does not
    /// recognise, kept so a save re-emits them: a zone field added by a
    /// later build (as `holes` was in #818) survives a re-save here (#883).
    ///
    /// Unlike `Pad` and `Footprint`, whose foreign keys come from an
    /// `[extras.*]` table, a zone is its own TOML table, so the bag is
    /// flattened straight into it. Two side effects on the JSON path (tests
    /// only today): the foreign keys serialise at the zone's top level rather
    /// than under `"unknown"`, and a `null` foreign value does not
    /// deserialise, since TOML has no null.
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

// ---------------------------------------------------------------------------
// Board-level graphics and text
// ---------------------------------------------------------------------------

/// A board drawing on a non-copper layer: a line, arc, circle, rectangle or
/// text frame (#2424; kinds in [`crate::pcb_graphic`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardGraphic {
    /// Shares the board's one id space with the other board items. A file
    /// written before #2424 has none; loading derives one per drawing.
    #[serde(default)]
    pub uuid: Uuid,
    #[serde(default)]
    pub graphic_type: String,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub width: f64,
    pub start: Option<Point>,
    pub end: Option<Point>,
    pub center: Option<Point>,
    #[serde(default)]
    pub radius: f64,
    #[serde(default)]
    pub points: Vec<Point>,
    /// A text frame's text; `\n` breaks a line. Not written when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// A text frame's text height in mm. Not written when 0.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub font_size: f64,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardText {
    pub uuid: Uuid,
    #[serde(default)]
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub layer: String,
    #[serde(default)]
    pub font_size: f64,
    #[serde(default)]
    pub rotation: f64,
}

// ---------------------------------------------------------------------------
// Top-level board
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcbBoard {
    pub uuid: Uuid,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub thickness: f64,
    #[serde(default)]
    pub outline: Vec<Point>,
    #[serde(default)]
    pub layers: Vec<LayerDef>,
    pub setup: Option<PcbSetup>,
    #[serde(default)]
    pub nets: Vec<NetDef>,
    #[serde(default)]
    pub footprints: Vec<Footprint>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub vias: Vec<Via>,
    #[serde(default)]
    pub zones: Vec<Zone>,
    #[serde(default)]
    pub graphics: Vec<BoardGraphic>,
    #[serde(default)]
    pub texts: Vec<BoardText>,
    /// Free arcs, pads and dimensions (#2373).
    #[serde(
        default,
        skip_serializing_if = "crate::pcb_free::FreePrimitives::is_empty"
    )]
    pub free: crate::pcb_free::FreePrimitives,
    /// The board sketch (#2234) and the keepouts, cutouts, holes and
    /// V-scores it baked to; `None` for a board drawn without one. Written
    /// only through the board engine's `SetBoardSketch` command, which also
    /// writes [`Self::outline`] when the sketch owns it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<crate::pcb_sketch::BoardSketch>,
    /// Guides, extra grids, the sheet and naming options (#2374). Written
    /// only through the board engine's `SetBoardOptions` command.
    #[serde(
        default,
        skip_serializing_if = "crate::pcb_options::BoardOptions::is_default"
    )]
    pub options: crate::pcb_options::BoardOptions,
    /// The physical layer stack (#2401); `None` for a board whose stack was
    /// never edited, which shows [`LayerStack::derived`]. Written only
    /// through the board engine's `SetLayerStack` command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stackup: Option<LayerStack>,
    /// Rooms (#2457): named areas holding the parts of a sheet placement.
    /// Written only through the board engine's room commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rooms: Vec<Room>,
    /// Keys in the `.snxpcb` manifest, `[stackup]`, `[nets]` and
    /// `[extras.board]` that this build does not recognise, kept per
    /// section so a save re-emits each where it was found. Empty for
    /// every file written by this build.
    #[serde(default, skip_serializing_if = "BoardUnknown::is_empty")]
    pub unknown: BoardUnknown,
}

impl PcbBoard {
    /// Take what loading repaired in this board (see
    /// [`SheetLoadRepair`](crate::format::SheetLoadRepair)), leaving none
    /// behind: the opener reports it once, and later reads never carry it.
    pub fn take_load_repairs(&mut self) -> Vec<crate::format::SheetLoadRepair> {
        std::mem::take(&mut self.unknown.load_repairs)
    }
}

// ---------------------------------------------------------------------------
// Typed layer accessors (#2173)
// ---------------------------------------------------------------------------

// Typed views of the layer strings. `None` is a name this build does not
// know; the field keeps it as written, so a save re-emits it unchanged.
macro_rules! impl_pcb_layer {
    ($($ty:ty => $field:ident),+ $(,)?) => {
        $(impl $ty {
            /// The typed layer, or `None` for a name this build does not know.
            pub fn pcb_layer(&self) -> Option<PcbLayer> {
                parse_pcb_layer(&self.$field)
            }
        })+
    };
}

macro_rules! impl_pcb_layers {
    ($($ty:ty),+ $(,)?) => {
        $(impl $ty {
            /// The typed layers in order; `None` for a name this build does
            /// not know.
            pub fn pcb_layers(&self) -> impl Iterator<Item = Option<PcbLayer>> + '_ {
                self.layers.iter().map(|name| parse_pcb_layer(name))
            }
        })+
    };
}

impl_pcb_layer!(
    LayerDef => name,
    FpGraphic => layer,
    Footprint => layer,
    Segment => layer,
    Zone => layer,
    BoardGraphic => layer,
    BoardText => layer,
);

impl_pcb_layers!(Pad, Via);
