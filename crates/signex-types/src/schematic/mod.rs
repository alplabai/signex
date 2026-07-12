use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::property::SchematicProperty;

// ---------------------------------------------------------------------------
// Schematic text constants
// ---------------------------------------------------------------------------

/// Default schematic text height: 1.27 mm = 50 mils = 10 Altium pt.
pub const SCHEMATIC_TEXT_MM: f64 = 1.27;

/// Altium schematic point → mm: 1 pt = 0.127 mm (10 pt = 1.27 mm).
pub const SCHEMATIC_PT_TO_MM: f64 = 0.127;

/// Schematic coarse grid step: 2.54 mm = 100 mils. Used as pin length,
/// bus-entry size, and any other default that snaps to the coarse grid.
/// Matches the long-standing EDA convention of 100-mil pin grids.
pub const GRID_MM: f64 = 2.54;

/// Default pin line length in mm (one coarse-grid step).
pub const PIN_LENGTH_MM: f64 = GRID_MM;

/// Default offset from pin body-end to pin name text anchor.
pub const PIN_NAME_OFFSET_MM: f64 = 0.508;

/// Default screen-space stroke width for schematic pin lines (px).
/// Shared across symbol/schematic canvas paths to avoid drift.
pub const PIN_STROKE_PX: f32 = 3.0;

/// Stroke width for selected schematic pin lines (px).
pub const PIN_STROKE_SELECTED_PX: f32 = 5.0;

/// Camera scale value that maps to 100% zoom in schematic canvases.
pub const SCHEMATIC_ZOOM_100_SCALE: f32 = 3.0;

/// Shared minimum stroke width for world-space schematic primitives (mm).
pub const SCHEMATIC_RENDER_MIN_STROKE_MM: f64 = 0.15;

/// Shared minimum visible stroke width after world->screen scaling (px).
pub const SCHEMATIC_RENDER_MIN_STROKE_PX: f32 = 0.6;

/// Maximum multiplier applied to 100% stroke widths at high zoom.
///
/// This keeps stroke growth monotonic with zoom while avoiding blocky,
/// over-thick lines at very high zoom levels.
pub const SCHEMATIC_RENDER_STROKE_MAX_SCALE_MULTIPLIER: f32 = 2.0;

/// Bus stroke width in world-space mm.
pub const SCHEMATIC_RENDER_BUS_STROKE_MM: f64 = 0.45;

/// Power-port preview body outline stroke at 100% zoom (px).
pub const SCHEMATIC_RENDER_POWER_PORT_STROKE_PX: f32 = 1.2;

/// Selection overlay threshold under which we render a dot marker (px).
pub const SCHEMATIC_RENDER_SELECTION_MARKER_THRESHOLD_PX: f32 = 2.0;

/// Selection overlay dot marker radius (px).
pub const SCHEMATIC_RENDER_SELECTION_MARKER_RADIUS_PX: f32 = 5.5;

/// Selection overlay marker stroke at 100% zoom (px).
pub const SCHEMATIC_RENDER_SELECTION_MARKER_STROKE_PX: f32 = 1.4;

/// Selection overlay rectangle stroke at 100% zoom (px).
pub const SCHEMATIC_RENDER_SELECTION_RECT_STROKE_PX: f32 = 1.2;

/// Wire hit-test absolute minimum tolerance (mm).
pub const SCHEMATIC_HIT_WIRE_TOLERANCE_MM: f64 = 0.25;

/// Bus hit-test tolerance (mm).
pub const SCHEMATIC_HIT_BUS_TOLERANCE_MM: f64 = 0.55;

/// Junction render minimum radius (mm).
pub const SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM: f64 = 0.35;

/// No-connect marker half arm length (mm).
pub const SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM: f64 = 0.7;

/// No-connect marker minimum half arm length after scaling (px).
pub const SCHEMATIC_RENDER_NO_CONNECT_MIN_HALF_LEN_PX: f32 = 3.0;

/// No-connect marker stroke width at 100% zoom (px).
pub const SCHEMATIC_RENDER_NO_CONNECT_STROKE_PX: f32 = 1.2;

/// Symbol body rectangle stroke width at 100% zoom (px).
pub const SCHEMATIC_RENDER_SYMBOL_BODY_STROKE_PX: f32 = 1.1;

/// Child sheet rectangle stroke width at 100% zoom (px).
pub const SCHEMATIC_RENDER_CHILD_SHEET_STROKE_PX: f32 = 1.0;

/// Child sheet pin marker radius (px).
pub const SCHEMATIC_RENDER_CHILD_SHEET_PIN_RADIUS_PX: f32 = 3.0;

/// Drawing circle primitive minimum visible radius (px).
pub const SCHEMATIC_RENDER_DRAWING_MIN_CIRCLE_RADIUS_PX: f32 = 0.7;

/// Drawing arc primitive minimum visible radius (px).
pub const SCHEMATIC_RENDER_DRAWING_MIN_ARC_RADIUS_PX: f32 = 0.8;

/// Label glyph (global/hierarchical marker) outline stroke width at 100% zoom (px).
pub const SCHEMATIC_RENDER_LABEL_GLYPH_STROKE_PX: f32 = 1.0;

// ---------------------------------------------------------------------------
// Point — schematic mm-space coordinate (f64). Copy-friendly.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }
}

impl Default for Point {
    fn default() -> Self {
        Point::ZERO
    }
}

// ---------------------------------------------------------------------------
// SymbolTransform — Y-up library → Y-down schematic placement
// ---------------------------------------------------------------------------
//
// HI-19: shared between `signex-render` (folds parent transform into pin /
// field positions at draw time) and `signex-engine` (computes per-symbol
// world-space coordinates for hit-testing, autoplace, ERC). Lives here so
// both crates use ONE implementation; previously `signex-render` had a
// public `SymbolTransform::apply` and `signex-engine` had a private
// `transform_local_point` that recomputed the same math, opening the door
// to silent divergence on any future handedness or mirror-compose change.

/// World-space placement of a parent symbol — used when folding a
/// child element's library-space rotation/mirror with the parent's
/// transform.
///
/// The transform is `Y-up library` → `Y-down schematic`: a library-space
/// pin at `(0, +pin_length)` lands at world position `(0, -pin_length)`
/// relative to the parent body when the parent has no rotation or mirror.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolTransform {
    pub origin: Point,
    pub rotation_deg: f64,
    pub mirror_x: bool,
    pub mirror_y: bool,
}

impl SymbolTransform {
    /// Build from a placed `Symbol`.
    #[inline]
    pub fn from_symbol(symbol: &Symbol) -> Self {
        Self {
            origin: symbol.position,
            rotation_deg: symbol.rotation,
            mirror_x: symbol.mirror_x,
            mirror_y: symbol.mirror_y,
        }
    }

    /// Apply transform to a library-space point.
    #[must_use]
    pub fn apply(&self, local: Point) -> Point {
        let x = local.x;
        let y = -local.y;
        let rad = -self.rotation_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        let mut rx = x * cos - y * sin;
        let mut ry = x * sin + y * cos;
        if self.mirror_y {
            rx = -rx;
        }
        if self.mirror_x {
            ry = -ry;
        }
        Point::new(rx + self.origin.x, ry + self.origin.y)
    }

    /// Compose a child rotation (degrees, clockwise positive in
    /// schematic-screen space) with the parent's rotation + mirror so
    /// the rendered angle ends up correct.
    #[must_use]
    pub fn apply_angle(&self, child_deg: f64) -> f64 {
        let mut r = self.rotation_deg + child_deg;
        if self.mirror_x {
            r = -r + 180.0;
        }
        if self.mirror_y {
            r = -r;
        }
        r.rem_euclid(360.0)
    }
}

// ---------------------------------------------------------------------------
// Alignment enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VAlign {
    Top,
    #[default]
    Center,
    Bottom,
}

// ---------------------------------------------------------------------------
// Fill
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FillType {
    #[default]
    None,
    Outline,
    Background,
}

// ---------------------------------------------------------------------------
// Pin types — Signex-curated, not derived from any specific EDA enum.
// See crates/signex-types/docs/pin-design.md for the rationale behind
// every variant choice (size, boundaries, names).
// ---------------------------------------------------------------------------

/// Pin electrical role.
///
/// Curated 14-variant set spanning generic digital pins, power pins,
/// open-drain polarity-tagged outputs, plus Signex-original additions
/// (`GroundReference`, `Differential`, `Clock`) that don't appear in
/// other EDA tools' enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    /// Drives signal in.
    Input,
    /// Drives signal out.
    Output,
    /// Drives signal both ways depending on context.
    Bidirectional,
    /// Tri-statable output — can be high-Z.
    ThreeStatable,
    /// Passive electrical (resistor / capacitor / inductor terminal).
    Passive,
    /// Power supply input pin.
    PowerInput,
    /// Power supply output pin (regulator output, battery positive, etc.).
    PowerOutput,
    /// Ground reference — Signex-original, distinguishes ground from generic power.
    GroundReference,
    /// Open-drain / open-collector, active-low output.
    OpenDrainLow,
    /// Open-drain / open-emitter, active-high output.
    OpenDrainHigh,
    /// Differential pair member — Signex-original (HSD-friendly).
    Differential,
    /// Clock pin — Signex-original (modeled as a direction, not a shape).
    Clock,
    /// Pin must remain unconnected (manufacturer-marked NC).
    DoNotConnect,
    /// Author has not classified the pin yet (default for new pins);
    /// collapses what other EDA tools sometimes split into "free" vs
    /// "unspecified".
    Unclassified,
}

/// Pin graphic decoration on the symbol pin tip.
///
/// 7 variants — drops the per-direction "low" shape modifiers that
/// other EDA tools include (since `PinDirection`'s `OpenDrainLow` /
/// `OpenDrainHigh` carry that information already). Adds Schmitt /
/// Hysteresis as Signex-original variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinShapeStyle {
    Plain,
    InvertedBubble,
    ClockTriangle,
    InvertedClockBubble,
    HysteresisInput,
    HysteresisOutput,
    Schmitt,
}

// ---------------------------------------------------------------------------
// Label type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelType {
    Net,
    Global,
    Hierarchical,
    Power,
}

// ---------------------------------------------------------------------------
// Text property (for reference/value fields)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextProp {
    pub position: Point,
    pub rotation: f64,
    pub font_size: f64,
    #[serde(default)]
    pub justify_h: HAlign,
    #[serde(default)]
    pub justify_v: VAlign,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolInstance {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default = "default_unit")]
    pub unit: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SheetInstance {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub page: String,
}

// ---------------------------------------------------------------------------
// LibSymbol & graphics
// ---------------------------------------------------------------------------

/// A graphic primitive inside a library symbol, tagged with unit and body-style
/// so the renderer can filter to only draw the correct unit for each instance.
///
/// - `unit == 0`       → common to ALL units (always rendered)
/// - `unit == N`       → only rendered for symbol instances with `unit = N`
/// - `body_style == 0` → common to all body styles (normal + De Morgan)
/// - `body_style == 1` → normal body style (default)
/// - `body_style == 2` → De Morgan body style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibGraphic {
    #[serde(default)]
    pub unit: u32,
    #[serde(default = "default_body_style")]
    pub body_style: u32,
    pub graphic: Graphic,
}

/// A pin inside a library symbol, tagged with unit and body-style.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibPin {
    #[serde(default)]
    pub unit: u32,
    #[serde(default = "default_body_style")]
    pub body_style: u32,
    pub pin: Pin,
}

fn default_body_style() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibSymbol {
    pub id: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint: String,
    #[serde(default)]
    pub datasheet: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keywords: String,
    #[serde(default)]
    pub fp_filters: String,
    #[serde(default = "default_true")]
    pub in_bom: bool,
    #[serde(default = "default_true")]
    pub on_board: bool,
    #[serde(default = "default_true")]
    pub in_pos_files: bool,
    #[serde(default)]
    pub duplicate_pin_numbers_are_jumpers: bool,
    #[serde(default)]
    pub graphics: Vec<LibGraphic>,
    #[serde(default)]
    pub pins: Vec<LibPin>,
    #[serde(default = "default_true")]
    pub show_pin_numbers: bool,
    #[serde(default = "default_true")]
    pub show_pin_names: bool,
    #[serde(default)]
    pub pin_name_offset: f64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Graphic {
    Polyline {
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Rectangle {
        start: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Circle {
        center: Point,
        radius: f64,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    Text {
        text: String,
        position: Point,
        #[serde(default)]
        rotation: f64,
        #[serde(default)]
        font_size: f64,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        justify_h: HAlign,
        #[serde(default)]
        justify_v: VAlign,
    },
    TextBox {
        text: String,
        position: Point,
        #[serde(default)]
        rotation: f64,
        size: Point,
        #[serde(default)]
        font_size: f64,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        italic: bool,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
    /// Cubic bezier: control points [p0, c1, c2, p3]
    Bezier {
        /// Exactly 4 control points: start, cp1, cp2, end
        points: Vec<Point>,
        #[serde(default)]
        width: f64,
        #[serde(default)]
        fill: FillType,
    },
}

// ---------------------------------------------------------------------------
// Pin
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pin {
    pub direction: PinDirection,
    pub shape_style: PinShapeStyle,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub length: f64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub number: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default = "default_true")]
    pub name_visible: bool,
    #[serde(default = "default_true")]
    pub number_visible: bool,
}

// ---------------------------------------------------------------------------
// Symbol instance
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub uuid: Uuid,
    pub lib_id: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub footprint: String,
    #[serde(default)]
    pub datasheet: String,
    pub position: Point,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub mirror_x: bool,
    #[serde(default)]
    pub mirror_y: bool,
    #[serde(default = "default_unit")]
    pub unit: u32,
    #[serde(default)]
    pub is_power: bool,
    pub ref_text: Option<TextProp>,
    pub val_text: Option<TextProp>,
    #[serde(default)]
    pub fields_autoplaced: bool,
    /// `true` when the user has manually placed at least one field on
    /// this symbol; the autoplacer will skip the symbol so user
    /// positioning is never silently overwritten on a subsequent
    /// rotate / mirror. Set when a field is dragged or has its
    /// rotation manually edited.
    #[serde(default)]
    pub fields_user_placed: bool,
    #[serde(default)]
    pub dnp: bool,
    #[serde(default = "default_true")]
    pub in_bom: bool,
    #[serde(default = "default_true")]
    pub on_board: bool,
    #[serde(default)]
    pub exclude_from_sim: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub fields: HashMap<String, String>,
    #[serde(default)]
    pub custom_properties: Vec<SchematicProperty>,
    #[serde(default)]
    pub pin_uuids: HashMap<String, Uuid>,
    #[serde(default)]
    pub instances: Vec<SymbolInstance>,

    // ─────────────────────────────────────────────────────────────────────
    // v0.9 §3.5 — schematic-side library pinning
    //
    // When a Symbol is placed from a `*.snxlib/` row (Library panel /
    // picker), the dispatcher tags it with the row's identity + version
    // so re-opening the schematic can detect drift against the library's
    // current row version.  All three fields are `#[serde(default)]` so
    // legacy `.snxsch` files (and any sheet imported from a foreign
    // format that has no notion of a Signex library) load cleanly with
    // `library_id = None`, `row_id = None`, `version = ""`.
    // ─────────────────────────────────────────────────────────────────────
    /// Source library for this placed Symbol.  `None` for sheets imported
    /// from a foreign format, hand-built primitives, or any Symbol that
    /// wasn't placed via a `.snxlib` row.
    #[serde(default)]
    pub library_id: Option<Uuid>,
    /// Row identity inside the source library — points at the
    /// `ComponentRow.row_id` so renaming the row's `internal_pn` doesn't
    /// break placement.  `None` whenever `library_id` is `None`.
    #[serde(default)]
    pub row_id: Option<Uuid>,
    /// Pinned row version at place-time (semver-style, e.g. `"1.0.2"`).
    /// Empty for un-pinned placements.  The Library Updates dialog
    /// (`v0.9-snxlib-as-file-plan.md` §3.5) compares this to the row's
    /// current `ComponentRow.version` on schematic open.
    #[serde(default)]
    pub library_version: String,
}

fn default_unit() -> u32 {
    1
}

// ---------------------------------------------------------------------------
// Wiring primitives
mod sheet;
pub use sheet::*;

#[cfg(test)]
mod symbol_transform_tests {
    use super::{Point, SymbolTransform};

    fn xform(rotation_deg: f64, mirror_x: bool, mirror_y: bool) -> SymbolTransform {
        SymbolTransform {
            origin: Point::ZERO,
            rotation_deg,
            mirror_x,
            mirror_y,
        }
    }

    fn assert_pt(got: Point, ex: f64, ey: f64) {
        assert!(
            (got.x - ex).abs() < 1e-9 && (got.y - ey).abs() < 1e-9,
            "expected ({ex}, {ey}), got ({}, {})",
            got.x,
            got.y
        );
    }

    #[test]
    fn identity_flips_library_y_up_to_schematic_y_down() {
        // No rotation or mirror: the only change is the Y axis flip
        // (library Y-up → schematic Y-down); X passes through.
        let t = xform(0.0, false, false);
        assert_pt(t.apply(Point::new(0.0, 10.0)), 0.0, -10.0);
        assert_pt(t.apply(Point::new(10.0, 0.0)), 10.0, 0.0);
    }

    #[test]
    fn origin_translates_the_result() {
        let t = xform(0.0, false, false);
        let t = SymbolTransform {
            origin: Point::new(5.0, 5.0),
            ..t
        };
        assert_pt(t.apply(Point::new(10.0, 0.0)), 15.0, 5.0);
    }

    #[test]
    fn rotation_90_turns_the_axes() {
        let t = xform(90.0, false, false);
        assert_pt(t.apply(Point::new(10.0, 0.0)), 0.0, -10.0);
        assert_pt(t.apply(Point::new(0.0, 10.0)), -10.0, 0.0);
    }

    #[test]
    fn mirror_x_flips_the_y_output_mirror_y_flips_the_x() {
        assert_pt(
            xform(0.0, true, false).apply(Point::new(0.0, 10.0)),
            0.0,
            10.0,
        );
        assert_pt(
            xform(0.0, false, true).apply(Point::new(10.0, 0.0)),
            -10.0,
            0.0,
        );
    }

    #[test]
    fn rotation_and_mirror_compose() {
        // A 90°-rotated, X-mirrored symbol: a pin at (2.54, 0) projects to
        // (0, 2.54) — the placement exercised by the netlist equivalence gate.
        let t = xform(90.0, true, false);
        assert_pt(t.apply(Point::new(2.54, 0.0)), 0.0, 2.54);
    }

    #[test]
    fn apply_angle_folds_rotation_and_mirror() {
        assert!((xform(0.0, false, false).apply_angle(90.0) - 90.0).abs() < 1e-9);
        assert!((xform(90.0, false, false).apply_angle(0.0) - 90.0).abs() < 1e-9);
        // mirror_x: r = -(rot + child) + 180.
        assert!((xform(0.0, true, false).apply_angle(30.0) - 150.0).abs() < 1e-9);
        // mirror_y: r = -(rot + child), wrapped into [0, 360).
        assert!((xform(0.0, false, true).apply_angle(30.0) - 330.0).abs() < 1e-9);
    }
}
