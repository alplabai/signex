//! Placed schematic-sheet elements and the sheet container.

use super::*;
use crate::format::{SheetUnknown, UnknownColumns};

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
    /// Stroke width in mm. 0.0 = use schematic default (~0.15mm).
    #[serde(default)]
    pub stroke_width: f64,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Junction {
    pub uuid: Uuid,
    pub position: Point,
    /// 0.0 means use the theme default size.
    #[serde(default)]
    pub diameter: f64,
    /// `true` when this dot was minted by the wire-junction autoplacer
    /// (`reconcile_wire_junctions` / `junctions_for_wire` in
    /// `signex-engine`) rather than placed by the user. Minted dots are
    /// re-validated on every wire-geometry command and removed once no
    /// wire meeting justifies them any more; a user-placed dot — which
    /// includes every junction in a `.snxsch` written before this field
    /// existed, via the `#[serde(default)]` below — is left alone even
    /// if it no longer sits on a wire (issue #422).
    #[serde(default)]
    pub minted: bool,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub uuid: Uuid,
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    pub label_type: LabelType,
    #[serde(default)]
    pub shape: String,
    #[serde(default)]
    pub font_size: f64,
    /// Font family the author chose for this label, `None` to inherit the
    /// sheet default (see [`crate::font`]). On disk it is an optional
    /// `font_family` TSV column, present only when some label sets one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default)]
    pub justify: HAlign,
    #[serde(default = "default_label_v_align")]
    pub justify_v: VAlign,
    /// TSV columns this build does not recognise, kept per row so a
    /// save re-emits them (see [`UnknownColumns`]).
    #[serde(default, skip_serializing_if = "UnknownColumns::is_empty")]
    pub unknown_columns: UnknownColumns,
}

impl Label {
    /// The sheet-port name this label exposes to a parent sheet, or `None`
    /// when it is not a port label (only Hierarchical and Global labels cross
    /// a sheet boundary) or its text is blank. The single predicate +
    /// normaliser the engine's port collector, the netlist stitcher and the
    /// ERC hierarchy rule all share (#359).
    pub fn port_name(&self) -> Option<&str> {
        sheet_port_name(self.label_type, &self.text)
    }
}

/// [`Label::port_name`] for callers that hold a label's kind and text
/// separately (e.g. the ERC snapshot).
pub fn sheet_port_name(label_type: LabelType, text: &str) -> Option<&str> {
    match label_type {
        LabelType::Hierarchical | LabelType::Global => normalize_port_name(text),
        LabelType::Net | LabelType::Power => None,
    }
}

/// Canonical form of a sheet port / sheet pin name: surrounding whitespace is
/// not part of the name, and a blank name is no name at all.
pub fn normalize_port_name(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn default_label_v_align() -> VAlign {
    VAlign::Bottom
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoConnect {
    pub uuid: Uuid,
    pub position: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextNote {
    pub uuid: Uuid,
    pub text: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub font_size: f64,
    /// Font family the author chose for this text, `None` to inherit the
    /// sheet default (see [`crate::font`]). Never rewritten when the family
    /// is not installed: resolution happens at render time only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default)]
    pub justify_h: HAlign,
    #[serde(default)]
    pub justify_v: VAlign,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bus {
    pub uuid: Uuid,
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusEntry {
    pub uuid: Uuid,
    pub position: Point,
    pub size: (f64, f64),
}

// ---------------------------------------------------------------------------
// Hierarchical sheets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetPin {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default)]
    pub direction: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub auto_generated: bool,
    #[serde(default)]
    pub user_moved: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildSheet {
    pub uuid: Uuid,
    pub name: String,
    pub filename: String,
    pub position: Point,
    pub size: (f64, f64),
    #[serde(default)]
    pub stroke_width: f64,
    #[serde(default)]
    pub fill: FillType,
    /// Optional outline colour parsed from `(stroke (color r g b a))`.
    /// `None` means "use the renderer's default for the active style".
    #[serde(default)]
    pub stroke_color: Option<StrokeColor>,
    /// Optional body fill colour parsed from `(fill (color r g b a))`.
    /// `None` means "use the renderer's default for the active style".
    #[serde(default)]
    pub fill_color: Option<StrokeColor>,
    #[serde(default)]
    pub fields_autoplaced: bool,
    #[serde(default)]
    pub pins: Vec<SheetPin>,
    #[serde(default)]
    pub instances: Vec<SheetInstance>,
}

// ---------------------------------------------------------------------------
// Schematic drawing primitives
// ---------------------------------------------------------------------------

/// Optional RGBA override for an individual `SchDrawing`. `None` means
/// "use the theme's default drawing colour" — the renderer falls back to
/// `CanvasColors.outline`. Stored per-drawing so users can recolour
/// individual shapes without disturbing the sheet theme.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StrokeColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SchDrawing {
    Line {
        uuid: Uuid,
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        stroke_color: Option<StrokeColor>,
    },
    Rect {
        uuid: Uuid,
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
        #[serde(default)]
        stroke_color: Option<StrokeColor>,
    },
    Circle {
        uuid: Uuid,
        center: Point,
        radius: f64,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
        #[serde(default)]
        stroke_color: Option<StrokeColor>,
    },
    Arc {
        uuid: Uuid,
        start: Point,
        mid: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
        #[serde(default)]
        stroke_color: Option<StrokeColor>,
    },
    Polyline {
        uuid: Uuid,
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
        #[serde(default)]
        stroke_color: Option<StrokeColor>,
    },
}

// ---------------------------------------------------------------------------
// Library symbol table
// ---------------------------------------------------------------------------

/// The sheet's embedded library symbols, keyed by lib id.
///
/// Reference-counted so a sheet copy (the per-frame drag preview, the engine's
/// undo snapshot) shares the table instead of deep-copying every `LibSymbol`.
/// Reads go through [`Deref`](std::ops::Deref); the first mutation of a shared
/// table copies it ([`std::sync::Arc::make_mut`]), so an edit never shows in
/// another copy. Serialises exactly like the plain map.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LibSymbolTable(std::sync::Arc<HashMap<String, LibSymbol>>);

impl LibSymbolTable {
    /// True when both tables are the same allocation (a copy that has not
    /// been mutated since).
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::ops::Deref for LibSymbolTable {
    type Target = HashMap<String, LibSymbol>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for LibSymbolTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl FromIterator<(String, LibSymbol)> for LibSymbolTable {
    fn from_iter<I: IntoIterator<Item = (String, LibSymbol)>>(iter: I) -> Self {
        Self(std::sync::Arc::new(iter.into_iter().collect()))
    }
}

// ---------------------------------------------------------------------------
// Top-level sheet
// ---------------------------------------------------------------------------

impl SchDrawing {
    /// The drawing's own id, whichever shape it is.
    pub fn uuid(&self) -> Uuid {
        match self {
            Self::Line { uuid, .. }
            | Self::Rect { uuid, .. }
            | Self::Circle { uuid, .. }
            | Self::Arc { uuid, .. }
            | Self::Polyline { uuid, .. } => *uuid,
        }
    }
}

/// Whether a drawing paints under the circuit or over it (#1583).
///
/// Drawings are documentation, so the default keeps them behind symbols,
/// wires and pins; `Above` is for a highlight or a callout laid over the
/// circuit. Text notes are not drawings and always stay on top.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingPlacement {
    #[default]
    Behind,
    Above,
}

impl SchematicSheet {
    /// The placement of drawing `uuid`; [`DrawingPlacement::Behind`] when none
    /// was chosen.
    pub fn drawing_placement(&self, uuid: Uuid) -> DrawingPlacement {
        self.drawing_placements
            .get(&uuid)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchematicSheet {
    pub uuid: Uuid,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub generator: String,
    #[serde(default)]
    pub generator_version: String,
    #[serde(default)]
    pub paper_size: String,
    /// Sheet-wide default font family for every text item that names none.
    /// `None` means the bundled default (Iosevka). Stored under
    /// `[extras.sheet]` only when set, so legacy files stay byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default = "default_root_sheet_page")]
    pub root_sheet_page: String,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub wires: Vec<Wire>,
    #[serde(default)]
    pub junctions: Vec<Junction>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub child_sheets: Vec<ChildSheet>,
    #[serde(default)]
    pub no_connects: Vec<NoConnect>,
    #[serde(default)]
    pub text_notes: Vec<TextNote>,
    #[serde(default)]
    pub buses: Vec<Bus>,
    #[serde(default)]
    pub bus_entries: Vec<BusEntry>,
    #[serde(default)]
    pub drawings: Vec<SchDrawing>,
    #[serde(default)]
    pub no_erc_directives: Vec<NoConnect>,
    #[serde(default)]
    pub title_block: HashMap<String, String>,
    #[serde(default)]
    pub lib_symbols: LibSymbolTable,
    /// Net colour overrides set with the Active Bar net-colour pen, keyed by
    /// the uuid of the wire or junction they colour. Saved per sheet in
    /// `[extras.sheet.net_colors]`; empty (and not written) when the sheet
    /// has none. Overrides whose element no longer exists are pruned when
    /// the sheet is saved.
    #[serde(default)]
    pub net_colors: HashMap<Uuid, crate::theme::Color>,
    /// Where each drawing sits against the circuit (#1583), by drawing uuid.
    /// Only drawings that sit [`DrawingPlacement::Above`] have an entry: no
    /// entry means [`DrawingPlacement::Behind`], so a sheet that never chose
    /// one writes nothing and re-saves byte-identical. Saved in
    /// `[extras.sheet.drawing_placements]`.
    #[serde(default)]
    pub drawing_placements: BTreeMap<Uuid, DrawingPlacement>,
    /// Border, title block and zone counts of the sheet frame. Saved in
    /// `[extras.sheet.frame]` only when not the default.
    #[serde(default, skip_serializing_if = "SheetFrameSettings::is_default")]
    pub frame: SheetFrameSettings,
    /// Keys in the `.snxsch` manifest and `[extras.sheet]` that this
    /// build does not recognise, kept per section so a save re-emits
    /// each where it was found. Empty for every file written by this
    /// build.
    #[serde(default, skip_serializing_if = "SheetUnknown::is_empty")]
    pub unknown: SheetUnknown,
}

fn default_root_sheet_page() -> String {
    "1".to_string()
}

/// An empty sheet with a nil uuid; every other field equals its serde default
/// (pinned by `sheet_default_equals_deserialized_empty_manifest`).
/// For fixtures and partial literals; production code builds sheets
/// exhaustively and mints real uuids.
impl Default for SchematicSheet {
    fn default() -> Self {
        Self {
            uuid: Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            font_family: None,
            root_sheet_page: default_root_sheet_page(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: LibSymbolTable::default(),
            net_colors: HashMap::new(),
            drawing_placements: BTreeMap::new(),
            frame: SheetFrameSettings::default(),
            unknown: SheetUnknown::default(),
        }
    }
}

/// Zero-length wire at the origin with a nil uuid. For fixtures and partial
/// literals; production code builds wires exhaustively and mints real uuids.
impl Default for Wire {
    fn default() -> Self {
        Self {
            uuid: Uuid::nil(),
            start: Point::default(),
            end: Point::default(),
            stroke_width: 0.0,
            unknown_columns: UnknownColumns::default(),
        }
    }
}

/// Junction at the origin with a nil uuid. For fixtures and partial literals;
/// production code builds junctions exhaustively and mints real uuids.
impl Default for Junction {
    fn default() -> Self {
        Self {
            uuid: Uuid::nil(),
            position: Point::default(),
            diameter: 0.0,
            minted: false,
            unknown_columns: UnknownColumns::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Selection -- identifies what the user has selected on the canvas
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectedKind {
    Symbol,
    Wire,
    Bus,
    BusEntry,
    Junction,
    NoConnect,
    /// Generic No-ERC directive (`SchematicSheet::no_erc_directives`).
    NoErcDirective,
    Label,
    /// Hierarchical sheet pin rendered on a child-sheet symbol.
    SheetPin,
    TextNote,
    ChildSheet,
    Drawing,
    /// Symbol reference field ("C39", "R1", …). UUID = symbol UUID.
    SymbolRefField,
    /// Symbol value field ("100n", "10k", …). UUID = symbol UUID.
    SymbolValField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SelectedItem {
    pub uuid: Uuid,
    pub kind: SelectedKind,
}

impl SelectedItem {
    pub fn new(uuid: Uuid, kind: SelectedKind) -> Self {
        Self { uuid, kind }
    }
}

// ---------------------------------------------------------------------------
// Bounding box helpers
// ---------------------------------------------------------------------------

/// Axis-aligned bounding box in world (mm) coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Aabb {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            min_x: x1.min(x2),
            min_y: y1.min(y2),
            max_x: x1.max(x2),
            max_y: y1.max(y2),
        }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    pub fn expand(&self, margin: f64) -> Self {
        Self {
            min_x: self.min_x - margin,
            min_y: self.min_y - margin,
            max_x: self.max_x + margin,
            max_y: self.max_y + margin,
        }
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

impl SchematicSheet {
    /// True when the sheet was written by a newer Signex than this build.
    /// It loads for viewing but must not be edited or saved: the writer would
    /// drop content this build has no field for. The one rule every edit,
    /// save and bulk pass asks.
    pub fn is_read_only(&self) -> bool {
        self.unknown.newer_minor_source.is_some()
    }

    /// The typed paper size and orientation stored in `paper_size`.
    pub fn sheet_size(&self) -> (super::SheetSize, super::SheetOrientation) {
        super::SheetSize::parse(&self.paper_size)
    }

    /// Page `(width_mm, height_mm)` of this sheet; the single reading the
    /// canvas frame and the PDF page share.
    pub fn paper_mm(&self) -> (f64, f64) {
        let (size, orientation) = self.sheet_size();
        size.dimensions_mm(orientation)
    }

    /// Compute the bounding box of all elements in the sheet.
    pub fn content_bounds(&self) -> Option<Aabb> {
        let mut aabb: Option<Aabb> = None;

        let mut extend = |x: f64, y: f64| {
            aabb = Some(match aabb {
                Some(a) => Aabb {
                    min_x: a.min_x.min(x),
                    min_y: a.min_y.min(y),
                    max_x: a.max_x.max(x),
                    max_y: a.max_y.max(y),
                },
                None => Aabb::new(x, y, x, y),
            });
        };

        for s in &self.symbols {
            extend(s.position.x, s.position.y);
        }
        for w in &self.wires {
            extend(w.start.x, w.start.y);
            extend(w.end.x, w.end.y);
        }
        for b in &self.buses {
            extend(b.start.x, b.start.y);
            extend(b.end.x, b.end.y);
        }
        for j in &self.junctions {
            extend(j.position.x, j.position.y);
        }
        for l in &self.labels {
            extend(l.position.x, l.position.y);
        }
        for n in &self.no_connects {
            extend(n.position.x, n.position.y);
        }
        for d in &self.no_erc_directives {
            extend(d.position.x, d.position.y);
        }
        for t in &self.text_notes {
            extend(t.position.x, t.position.y);
        }
        for c in &self.child_sheets {
            extend(c.position.x, c.position.y);
            extend(c.position.x + c.size.0, c.position.y + c.size.1);
        }

        // Add margin around content
        aabb.map(|a| a.expand(10.0))
    }
}

/// Distance from a point to a line segment.
pub fn point_to_segment_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-12 {
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x).powi(2) + (py - proj_y).powi(2)).sqrt()
}
