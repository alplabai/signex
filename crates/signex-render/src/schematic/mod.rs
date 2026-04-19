//! Schematic element rendering -- wires, symbols, labels, pins, etc.
//!
//! Each submodule handles rendering one schematic element type using
//! the Iced Canvas `Frame` API. All functions are pure: they take data,
//! a `ScreenTransform`, and colors, then draw onto the frame.

pub mod drawing;
pub mod hit_test;
pub mod junction;
pub mod label;
pub mod pin;
pub mod selection;
pub mod symbol;
pub mod text;
pub mod wire;

use iced::Rectangle;
use iced::widget::canvas;

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use signex_types::schematic::{
    Aabb, Bus, BusEntry, ChildSheet, Junction, Label, LabelType, LibSymbol, NoConnect, SchDrawing,
    SchematicSheet, Symbol, TextNote, Wire,
};
use signex_types::theme::CanvasColors;

use crate::PowerPortStyle;
use crate::colors::to_iced;

// ---------------------------------------------------------------------------
// ScreenTransform -- decouples rendering from the app-layer Camera
// ---------------------------------------------------------------------------

/// Converts world coordinates (mm) to screen pixels.
///
/// The app layer constructs this from its `Camera` before calling render
/// functions, so `signex-render` never depends on `signex-app`.
#[derive(Debug, Clone, Copy)]
pub struct ScreenTransform {
    /// Screen-pixel offset of the world origin.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Pixels per mm -- higher values mean more zoom.
    pub scale: f32,
}

/// Render-facing snapshot of the visible schematic.
///
/// This trims persistence and document metadata away from the canvas seam while
/// keeping the fields that rendering, hit-testing, and selection overlays need.
#[derive(Debug, Clone)]
pub struct SchematicRenderSnapshot {
    pub paper_size: String,
    pub symbols: Vec<Symbol>,
    pub wires: Vec<Wire>,
    pub junctions: Vec<Junction>,
    pub labels: Vec<Label>,
    pub child_sheets: Vec<ChildSheet>,
    pub no_connects: Vec<NoConnect>,
    pub text_notes: Vec<TextNote>,
    pub buses: Vec<Bus>,
    pub bus_entries: Vec<BusEntry>,
    pub drawings: Vec<SchDrawing>,
    pub lib_symbols: HashMap<String, LibSymbol>,
    content_bounds: Option<Aabb>,
}

impl SchematicRenderSnapshot {
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        Self {
            paper_size: sheet.paper_size.clone(),
            symbols: sheet.symbols.clone(),
            wires: sheet.wires.clone(),
            junctions: sheet.junctions.clone(),
            labels: sheet.labels.clone(),
            child_sheets: sheet.child_sheets.clone(),
            no_connects: sheet.no_connects.clone(),
            text_notes: sheet.text_notes.clone(),
            buses: sheet.buses.clone(),
            bus_entries: sheet.bus_entries.clone(),
            drawings: sheet.drawings.clone(),
            lib_symbols: sheet.lib_symbols.clone(),
            content_bounds: sheet.content_bounds(),
        }
    }

    pub fn content_bounds(&self) -> Option<Aabb> {
        self.content_bounds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderInvalidation(u16);

impl RenderInvalidation {
    pub const NONE: Self = Self(0);
    pub const SYMBOLS: Self = Self(1 << 0);
    pub const WIRES: Self = Self(1 << 1);
    pub const LABELS: Self = Self(1 << 2);
    pub const TEXT_NOTES: Self = Self(1 << 3);
    pub const BUSES: Self = Self(1 << 4);
    pub const BUS_ENTRIES: Self = Self(1 << 5);
    pub const JUNCTIONS: Self = Self(1 << 6);
    pub const NO_CONNECTS: Self = Self(1 << 7);
    pub const CHILD_SHEETS: Self = Self(1 << 8);
    pub const DRAWINGS: Self = Self(1 << 9);
    pub const LIB_SYMBOLS: Self = Self(1 << 10);
    pub const PAPER: Self = Self(1 << 11);
    pub const FULL: Self = Self(u16::MAX);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn affects_content_bounds(self) -> bool {
        self.intersects(
            Self::SYMBOLS
                | Self::WIRES
                | Self::LABELS
                | Self::TEXT_NOTES
                | Self::BUSES
                | Self::BUS_ENTRIES
                | Self::JUNCTIONS
                | Self::NO_CONNECTS
                | Self::CHILD_SHEETS
                | Self::DRAWINGS
                | Self::FULL,
        )
    }
}

impl std::ops::BitOr for RenderInvalidation {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for RenderInvalidation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Default)]
pub struct PreparedPreviewGeometry {
    symbol_positions: HashMap<Uuid, (f32, f32)>,
    wire_midpoints: HashMap<Uuid, (f32, f32)>,
    label_positions: HashMap<Uuid, (f32, f32)>,
    symbol_reference_positions: HashMap<Uuid, (f32, f32)>,
    symbol_value_positions: HashMap<Uuid, (f32, f32)>,
}

impl PreparedPreviewGeometry {
    fn from_snapshot(snapshot: &SchematicRenderSnapshot) -> Self {
        let mut prepared = Self::default();
        prepared.refresh(snapshot, RenderInvalidation::FULL);
        prepared
    }

    fn refresh(&mut self, snapshot: &SchematicRenderSnapshot, invalidation: RenderInvalidation) {
        if invalidation.contains(RenderInvalidation::FULL)
            || invalidation
                .intersects(RenderInvalidation::SYMBOLS | RenderInvalidation::LIB_SYMBOLS)
        {
            self.symbol_positions = snapshot
                .symbols
                .iter()
                .map(|symbol| {
                    (
                        symbol.uuid,
                        (symbol.position.x as f32, symbol.position.y as f32),
                    )
                })
                .collect();
            self.symbol_reference_positions = snapshot
                .symbols
                .iter()
                .filter_map(|symbol| {
                    symbol.ref_text.as_ref().map(|prop| {
                        (
                            symbol.uuid,
                            (prop.position.x as f32, prop.position.y as f32),
                        )
                    })
                })
                .collect();
            self.symbol_value_positions = snapshot
                .symbols
                .iter()
                .filter_map(|symbol| {
                    symbol.val_text.as_ref().map(|prop| {
                        (
                            symbol.uuid,
                            (prop.position.x as f32, prop.position.y as f32),
                        )
                    })
                })
                .collect();
        }

        if invalidation.contains(RenderInvalidation::FULL)
            || invalidation.contains(RenderInvalidation::WIRES)
        {
            self.wire_midpoints = snapshot
                .wires
                .iter()
                .map(|wire| {
                    (
                        wire.uuid,
                        (
                            ((wire.start.x + wire.end.x) / 2.0) as f32,
                            ((wire.start.y + wire.end.y) / 2.0) as f32,
                        ),
                    )
                })
                .collect();
        }

        if invalidation.contains(RenderInvalidation::FULL)
            || invalidation.contains(RenderInvalidation::LABELS)
        {
            self.label_positions = snapshot
                .labels
                .iter()
                .map(|label| {
                    (
                        label.uuid,
                        (label.position.x as f32, label.position.y as f32),
                    )
                })
                .collect();
        }
    }

    pub fn symbol_position(&self, uuid: Uuid) -> Option<(f32, f32)> {
        self.symbol_positions.get(&uuid).copied()
    }

    pub fn wire_midpoint(&self, uuid: Uuid) -> Option<(f32, f32)> {
        self.wire_midpoints.get(&uuid).copied()
    }

    pub fn label_position(&self, uuid: Uuid) -> Option<(f32, f32)> {
        self.label_positions.get(&uuid).copied()
    }

    pub fn symbol_reference_position(&self, uuid: Uuid) -> Option<(f32, f32)> {
        self.symbol_reference_positions.get(&uuid).copied()
    }

    pub fn symbol_value_position(&self, uuid: Uuid) -> Option<(f32, f32)> {
        self.symbol_value_positions.get(&uuid).copied()
    }
}

/// Shared render-cache seam for the visible schematic.
///
/// The cache owns an `Arc` to the immutable render snapshot so canvas drawing,
/// hit-testing, and selection overlays can share one prepared view-model.
#[derive(Debug, Clone)]
pub struct SchematicRenderCache {
    snapshot: Arc<SchematicRenderSnapshot>,
    prepared_preview: PreparedPreviewGeometry,
}

impl SchematicRenderCache {
    pub fn from_sheet(sheet: &SchematicSheet) -> Self {
        let snapshot = SchematicRenderSnapshot::from_sheet(sheet);
        Self {
            prepared_preview: PreparedPreviewGeometry::from_snapshot(&snapshot),
            snapshot: Arc::new(snapshot),
        }
    }

    pub fn update_from_sheet(&mut self, sheet: &SchematicSheet, invalidation: RenderInvalidation) {
        if invalidation.contains(RenderInvalidation::FULL) {
            *self = Self::from_sheet(sheet);
            return;
        }

        let snapshot = Arc::make_mut(&mut self.snapshot);

        if invalidation.contains(RenderInvalidation::SYMBOLS) {
            snapshot.symbols = sheet.symbols.clone();
        }
        if invalidation.contains(RenderInvalidation::WIRES) {
            snapshot.wires = sheet.wires.clone();
        }
        if invalidation.contains(RenderInvalidation::LABELS) {
            snapshot.labels = sheet.labels.clone();
        }
        if invalidation.contains(RenderInvalidation::TEXT_NOTES) {
            snapshot.text_notes = sheet.text_notes.clone();
        }
        if invalidation.contains(RenderInvalidation::BUSES) {
            snapshot.buses = sheet.buses.clone();
        }
        if invalidation.contains(RenderInvalidation::BUS_ENTRIES) {
            snapshot.bus_entries = sheet.bus_entries.clone();
        }
        if invalidation.contains(RenderInvalidation::JUNCTIONS) {
            snapshot.junctions = sheet.junctions.clone();
        }
        if invalidation.contains(RenderInvalidation::NO_CONNECTS) {
            snapshot.no_connects = sheet.no_connects.clone();
        }
        if invalidation.contains(RenderInvalidation::CHILD_SHEETS) {
            snapshot.child_sheets = sheet.child_sheets.clone();
        }
        if invalidation.contains(RenderInvalidation::DRAWINGS) {
            snapshot.drawings = sheet.drawings.clone();
        }
        if invalidation.contains(RenderInvalidation::LIB_SYMBOLS) {
            snapshot.lib_symbols = sheet.lib_symbols.clone();
        }
        if invalidation.contains(RenderInvalidation::PAPER) {
            snapshot.paper_size = sheet.paper_size.clone();
        }
        if invalidation.affects_content_bounds() {
            snapshot.content_bounds = sheet.content_bounds();
        }

        self.prepared_preview.refresh(snapshot, invalidation);
    }

    pub fn snapshot(&self) -> &SchematicRenderSnapshot {
        self.snapshot.as_ref()
    }

    pub fn prepared_preview(&self) -> &PreparedPreviewGeometry {
        &self.prepared_preview
    }
}

impl ScreenTransform {
    /// Convert a world-space point (mm) to screen pixels.
    #[inline]
    pub fn world_to_screen(&self, x: f64, y: f64) -> (f32, f32) {
        (
            x as f32 * self.scale + self.offset_x,
            y as f32 * self.scale + self.offset_y,
        )
    }

    /// Convert a world-space distance (mm) to screen pixels.
    #[inline]
    pub fn world_len(&self, mm: f64) -> f32 {
        mm as f32 * self.scale
    }

    /// Return the iced `Point` for a world coordinate.
    #[inline]
    pub fn to_screen_point(&self, x: f64, y: f64) -> iced::Point {
        let (sx, sy) = self.world_to_screen(x, y);
        iced::Point::new(sx, sy)
    }
}

// ---------------------------------------------------------------------------
// Shared geometry helpers (used by symbol, drawing, hit_test)
// ---------------------------------------------------------------------------

/// Compute the circumscribed circle center and radius from three points.
/// Returns `None` if the points are collinear.
pub(super) fn circle_from_three_points(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
) -> Option<(f64, f64, f64)> {
    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if d.abs() < 1e-10 {
        return None;
    }
    let ux = ((x1 * x1 + y1 * y1) * (y2 - y3)
        + (x2 * x2 + y2 * y2) * (y3 - y1)
        + (x3 * x3 + y3 * y3) * (y1 - y2))
        / d;
    let uy = ((x1 * x1 + y1 * y1) * (x3 - x2)
        + (x2 * x2 + y2 * y2) * (x1 - x3)
        + (x3 * x3 + y3 * y3) * (x2 - x1))
        / d;
    let r = ((x1 - ux).powi(2) + (y1 - uy).powi(2)).sqrt();
    Some((ux, uy, r))
}

/// Check if `mid_angle` lies between `start_angle` and `end_angle` when
/// going counter-clockwise from start to end.
pub(super) fn is_angle_between_ccw(start: f64, mid: f64, end: f64) -> bool {
    let tau = std::f64::consts::TAU;
    let normalize = |a: f64| ((a % tau) + tau) % tau;
    let s = normalize(start);
    let m = normalize(mid);
    let e = normalize(end);
    if s <= e {
        s <= m && m <= e
    } else {
        m >= s || m <= e
    }
}

/// Return symbol field display position.
///
/// In our data model, field positions are stored as absolute schematic
/// coordinates from `.kicad_sch` and should be rendered directly.
pub(super) fn field_display_pos(
    prop_pos: &signex_types::schematic::Point,
    _sym: &signex_types::schematic::Symbol,
) -> (f64, f64) {
    (prop_pos.x, prop_pos.y)
}

/// Compute KiCad-like effective field draw properties under symbol transform.
///
/// Returns `(draw_rotation_deg, effective_h_align, effective_v_align)`.
///
/// KiCad stores `prop.rotation` in the symbol's lib frame. Compose with
/// `sym.rotation` to get the on-screen angle, then fold so text is always
/// drawn at 0° or 90° (readable — never upside-down or reversed):
///
/// * 180° → 0° with horizontal justify flipped
/// * 270° → 90° with vertical justify flipped
///
/// Mirror state additionally flips the perpendicular axis:
///
/// * `mirror_y` flips the X axis → toggle horizontal justify
/// * `mirror_x` flips the Y axis → toggle vertical justify
pub(super) fn field_effective_style(
    prop: &signex_types::schematic::TextProp,
    sym: &signex_types::schematic::Symbol,
) -> (
    f64,
    signex_types::schematic::HAlign,
    signex_types::schematic::VAlign,
) {
    use signex_types::schematic::{HAlign, VAlign};

    let total = (sym.rotation + prop.rotation).rem_euclid(360.0);
    let (draw_rot, fold_h, fold_v) = match total.round() as i32 {
        0 => (0.0, false, false),
        90 => (90.0, false, false),
        180 => (0.0, true, false),
        270 => (90.0, false, true),
        _ => (total, false, false),
    };

    let flip_h = fold_h ^ sym.mirror_y;
    let flip_v = fold_v ^ sym.mirror_x;

    let h = if flip_h {
        match prop.justify_h {
            HAlign::Left => HAlign::Right,
            HAlign::Right => HAlign::Left,
            HAlign::Center => HAlign::Center,
        }
    } else {
        prop.justify_h
    };
    let v = if flip_v {
        match prop.justify_v {
            VAlign::Top => VAlign::Bottom,
            VAlign::Bottom => VAlign::Top,
            VAlign::Center => VAlign::Center,
        }
    } else {
        prop.justify_v
    };

    (draw_rot, h, v)
}

/// Transform a local library-space point through a symbol instance's
/// position, rotation, and mirror state, returning a world-space point.
pub fn instance_transform(
    sym: &signex_types::schematic::Symbol,
    local: &signex_types::schematic::Point,
) -> (f64, f64) {
    // Step 1: Flip Y — KiCad library coords are Y-up, schematic is Y-down.
    let x = local.x;
    let y = -local.y;
    // Step 2: Rotate by NEGATIVE angle.
    let rad = -sym.rotation.to_radians();
    let cos = rad.cos();
    let sin = rad.sin();
    let rx = x * cos - y * sin;
    let ry = x * sin + y * cos;
    // Step 3: Mirror applied AFTER rotation (KiCad convention).
    let rx = if sym.mirror_y { -rx } else { rx };
    let ry = if sym.mirror_x { -ry } else { ry };
    // Step 4: Translate to world position.
    (rx + sym.position.x, ry + sym.position.y)
}

// ---------------------------------------------------------------------------
// Main render entry point
// ---------------------------------------------------------------------------

/// Draw all elements of a schematic sheet onto the canvas frame.
///
/// Elements are rendered in z-order so that higher-layer items paint
/// on top of lower ones.
pub fn render_schematic(
    frame: &mut canvas::Frame,
    sheet: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    _bounds: Rectangle,
    focus: Option<&std::collections::HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<
        &std::collections::HashMap<uuid::Uuid, signex_types::theme::Color>,
    >,
) {
    let body_color = to_iced(&colors.body);
    let body_fill_color = to_iced(&colors.body_fill);
    let wire_color = to_iced(&colors.wire);
    let junction_color = to_iced(&colors.junction);
    let pin_color = to_iced(&colors.pin);
    let reference_color = to_iced(&colors.reference);
    let value_color = to_iced(&colors.value);
    let no_connect_color = to_iced(&colors.no_connect);
    let bus_color = to_iced(&colors.bus);
    let power_color = to_iced(&colors.power);
    let power_style = crate::power_port_style();

    // AutoFocus: dim non-selected items so the focus set stands out.
    // When `focus` is None, every item draws at full alpha (normal mode).
    // When Some, items NOT in the set get their color alpha scaled to
    // `AUTO_FOCUS_DIM` so the selection reads as the spotlight. 0.28 is
    // low enough to visibly recede but high enough that the context is
    // still readable.
    const AUTO_FOCUS_DIM: f32 = 0.28;
    let alpha_for = |uuid: &uuid::Uuid| -> f32 {
        match focus {
            Some(set) if !set.contains(uuid) => AUTO_FOCUS_DIM,
            _ => 1.0,
        }
    };
    let dim = |c: iced::Color, a: f32| -> iced::Color { iced::Color { a: c.a * a, ..c } };

    // Z=1: Drawing primitives (lines, rects, circles, arcs, polylines)
    for d in &sheet.drawings {
        drawing::draw_sch_drawing(frame, d, transform, body_color);
    }

    // Z=2: Wires — per-wire colour overrides win over the theme wire
    // colour (net-colour flood from the Active Bar palette).
    for w in &sheet.wires {
        let base = wire_color_overrides
            .and_then(|o| o.get(&w.uuid))
            .map(to_iced)
            .unwrap_or(wire_color);
        wire::draw_wire(frame, w, transform, dim(base, alpha_for(&w.uuid)));
    }

    // Z=3: Buses
    for b in &sheet.buses {
        wire::draw_bus(frame, b, transform, dim(bus_color, alpha_for(&b.uuid)));
    }

    // Z=4: Bus entries
    for be in &sheet.bus_entries {
        wire::draw_bus_entry(frame, be, transform, dim(bus_color, alpha_for(&be.uuid)));
    }

    // Z=5: Junctions — honour per-uuid colour overrides so a junction
    // on a net-coloured net renders in the same colour as the wires.
    for j in &sheet.junctions {
        let base = wire_color_overrides
            .and_then(|o| o.get(&j.uuid))
            .map(to_iced)
            .unwrap_or(junction_color);
        junction::draw_junction(frame, j, transform, dim(base, alpha_for(&j.uuid)));
    }

    // Z=6: No-connect markers
    for nc in &sheet.no_connects {
        junction::draw_no_connect(
            frame,
            nc,
            transform,
            dim(no_connect_color, alpha_for(&nc.uuid)),
        );
    }

    // Z=7-9: Labels (net, global, hierarchical, power)
    for lbl in &sheet.labels {
        let color = match lbl.label_type {
            LabelType::Net => to_iced(&colors.net_label),
            LabelType::Global => to_iced(&colors.global_label),
            LabelType::Hierarchical => to_iced(&colors.hier_label),
            LabelType::Power => to_iced(&colors.power),
        };
        label::draw_label(
            frame,
            lbl,
            transform,
            dim(color, alpha_for(&lbl.uuid)),
            body_fill_color,
        );
    }

    // Z=10-11: Symbol bodies + pins
    for sym in &sheet.symbols {
        let a = alpha_for(&sym.uuid);
        let body_c = dim(body_color, a);
        let body_fill_c = dim(body_fill_color, a);
        let pin_c = dim(pin_color, a);
        let power_c = dim(power_color, a);
        let reference_c = dim(reference_color, a);
        let value_c = dim(value_color, a);
        if sym.is_power && matches!(power_style, PowerPortStyle::Altium) {
            draw_builtin_power(frame, sym, transform, power_c, power_c);
            continue;
        }

        if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) {
            symbol::draw_symbol(frame, sym, lib_sym, transform, body_c, body_fill_c, pin_c);

            // Pins
            pin::draw_symbol_pins(frame, sym, lib_sym, transform, pin_c);

            // Reference text — power symbols (#PWR refs) are always hidden
            if let Some(ref ref_text) = sym.ref_text
                && !ref_text.hidden
                && !sym.is_power
            {
                let dpos = field_display_pos(&ref_text.position, sym);
                text::draw_text_prop(
                    frame,
                    &sym.reference,
                    ref_text,
                    sym,
                    dpos,
                    transform,
                    reference_c,
                );
            }

            // Value text
            if let Some(ref val_text) = sym.val_text
                && !val_text.hidden
            {
                let dpos = field_display_pos(&val_text.position, sym);
                text::draw_text_prop(frame, &sym.value, val_text, sym, dpos, transform, value_c);
            }
        } else if sym.is_power {
            draw_builtin_power(frame, sym, transform, power_c, power_c);
        }
    }

    // Z=11b: Child sheets (hierarchical sheets)
    for child in &sheet.child_sheets {
        let a = alpha_for(&child.uuid);
        drawing::draw_child_sheet(
            frame,
            child,
            transform,
            dim(body_color, a),
            dim(body_fill_color, a),
        );
    }

    // Z=12: Text notes
    for tn in &sheet.text_notes {
        text::draw_text_note(
            frame,
            tn,
            transform,
            dim(to_iced(&colors.body), alpha_for(&tn.uuid)),
        );
    }
}

// ---------------------------------------------------------------------------
// Built-in Altium-style power symbol rendering
// ---------------------------------------------------------------------------

/// Draw a built-in power symbol when no lib_symbol definition exists.
/// Renders Altium-style shapes: GND (3 horizontal lines), VCC (bar + arrow),
/// Earth (diagonal hatch), Signal GND (triangle), generic (bar + label).
/// Public preview wrapper — renders the built-in power glyph at a ghost
/// color for placement previews (single color for both body and label).
pub fn draw_power_port_preview(
    frame: &mut canvas::Frame,
    sym: &signex_types::schematic::Symbol,
    transform: &ScreenTransform,
    color: iced::Color,
) {
    draw_builtin_power(frame, sym, transform, color, color);
}

fn draw_builtin_power(
    frame: &mut canvas::Frame,
    sym: &signex_types::schematic::Symbol,
    transform: &ScreenTransform,
    color: iced::Color,
    label_color: iced::Color,
) {
    use iced::widget::canvas::path;

    // Keep power symbol stroke consistent with normal wire width.
    let sw = transform.world_len(0.15).max(1.0);
    let stroke = canvas::Stroke::default().with_color(color).with_width(sw);

    // GND-like symbols point downward from anchor, VCC-like upward.
    let id = sym.lib_id.to_lowercase();
    let is_gnd_like = id.contains("gnd");
    let dir = if is_gnd_like { -1.0 } else { 1.0 };

    // Pin line: vertical stub from anchor toward symbol body.
    let pin_len = 1.27;
    let (p0x, p0y) = instance_transform(sym, &signex_types::schematic::Point::new(0.0, 0.0));
    let (p1x, p1y) = instance_transform(
        sym,
        &signex_types::schematic::Point::new(0.0, pin_len * dir),
    );
    let s0 = transform.to_screen_point(p0x, p0y);
    let s1 = transform.to_screen_point(p1x, p1y);
    frame.stroke(&canvas::Path::line(s0, s1), stroke);

    // Identify power type from lib_id or value
    let net = sym.value.to_uppercase();

    if id.contains("gnd") && !id.contains("earth") && !id.contains("gndref") {
        // GND: 3 horizontal lines of decreasing width (Altium uses 2.54 mm)
        let bar_w = 2.54;
        for (i, frac) in [1.0_f64, 0.65, 0.3].iter().enumerate() {
            let dy = (pin_len + 0.4 * i as f64) * dir;
            let hw = bar_w * 0.5 * frac;
            let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, dy));
            let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, dy));
            let sl = transform.to_screen_point(lx, ly);
            let sr = transform.to_screen_point(rx, ry);
            frame.stroke(&canvas::Path::line(sl, sr), stroke);
        }
    } else if id.contains("gndref") {
        // Signal GND: downward triangle
        let hw = 1.27;
        let tri_h = 1.27;
        let base_y = pin_len * dir;
        let pts = [
            signex_types::schematic::Point::new(-hw, base_y),
            signex_types::schematic::Point::new(hw, base_y),
            signex_types::schematic::Point::new(0.0, base_y + tri_h * dir),
        ];
        let screen_pts: Vec<iced::Point> = pts
            .iter()
            .map(|p| {
                let (wx, wy) = instance_transform(sym, p);
                transform.to_screen_point(wx, wy)
            })
            .collect();
        let tri = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(screen_pts[0]);
            b.line_to(screen_pts[1]);
            b.line_to(screen_pts[2]);
            b.close();
        });
        frame.stroke(&tri, stroke);
    } else if id.contains("earth") {
        // Earth: horizontal bar + 3 diagonal hatch lines (Altium 2.54 mm)
        let bar_w = 2.54;
        let base_y = pin_len * dir;
        let hw = bar_w * 0.5;
        let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, base_y));
        let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, base_y));
        frame.stroke(
            &canvas::Path::line(
                transform.to_screen_point(lx, ly),
                transform.to_screen_point(rx, ry),
            ),
            stroke,
        );
        // Diagonal hatch lines below bar
        for i in 0..3 {
            let x_off = -hw + (i as f64 + 0.5) * (bar_w / 3.0);
            let (hx1, hy1) =
                instance_transform(sym, &signex_types::schematic::Point::new(x_off, base_y));
            let (hx2, hy2) = instance_transform(
                sym,
                &signex_types::schematic::Point::new(x_off - 0.5, base_y + 0.8 * dir),
            );
            frame.stroke(
                &canvas::Path::line(
                    transform.to_screen_point(hx1, hy1),
                    transform.to_screen_point(hx2, hy2),
                ),
                stroke,
            );
        }
    } else if id.contains("arrow") {
        // Arrow: upward-pointing triangle at top of pin (Altium 2.54 mm base).
        let base_y = pin_len * dir;
        let tip_y = base_y + 1.4 * dir;
        let pts = [
            signex_types::schematic::Point::new(-1.27, base_y),
            signex_types::schematic::Point::new(1.27, base_y),
            signex_types::schematic::Point::new(0.0, tip_y),
        ];
        let screen_pts: Vec<iced::Point> = pts
            .iter()
            .map(|p| {
                let (wx, wy) = instance_transform(sym, p);
                transform.to_screen_point(wx, wy)
            })
            .collect();
        let tri = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(screen_pts[0]);
            b.line_to(screen_pts[2]);
            b.line_to(screen_pts[1]);
        });
        frame.stroke(&tri, stroke);
    } else if id.contains("wave") {
        // Wave: sinusoidal cap
        let base_y = pin_len * dir;
        let steps = 24_i32;
        let span = 2.6_f64;
        let amp = 0.5_f64;
        let mut pts: Vec<iced::Point> = Vec::with_capacity(steps as usize + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = -span / 2.0 + t * span;
            let y = base_y + dir * amp * (t * std::f64::consts::PI * 2.0).sin();
            let (wx, wy) = instance_transform(sym, &signex_types::schematic::Point::new(x, y));
            pts.push(transform.to_screen_point(wx, wy));
        }
        let path = canvas::Path::new(|b: &mut path::Builder| {
            b.move_to(pts[0]);
            for p in &pts[1..] {
                b.line_to(*p);
            }
        });
        frame.stroke(&path, stroke);
    } else if id.contains("circle") {
        // Circle: small open circle at pin top
        let base_y = pin_len * dir + 0.6 * dir;
        let (cx, cy) = instance_transform(sym, &signex_types::schematic::Point::new(0.0, base_y));
        let center = transform.to_screen_point(cx, cy);
        let r = transform.world_len(0.6).max(2.0);
        frame.stroke(&canvas::Path::circle(center, r), stroke);
    } else if id.contains("bar") {
        // Explicit "Bar" style — single horizontal bar (Altium convention).
        let bar_w = 2.54;
        let base_y = pin_len * dir;
        let hw = bar_w * 0.5;
        let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, base_y));
        let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, base_y));
        frame.stroke(
            &canvas::Path::line(
                transform.to_screen_point(lx, ly),
                transform.to_screen_point(rx, ry),
            ),
            stroke,
        );
    } else {
        // VCC / generic power: horizontal bar at top of pin (Altium 2.54 mm)
        let bar_w = 2.54;
        let base_y = pin_len * dir;
        let hw = bar_w * 0.5;
        let (lx, ly) = instance_transform(sym, &signex_types::schematic::Point::new(-hw, base_y));
        let (rx, ry) = instance_transform(sym, &signex_types::schematic::Point::new(hw, base_y));
        frame.stroke(
            &canvas::Path::line(
                transform.to_screen_point(lx, ly),
                transform.to_screen_point(rx, ry),
            ),
            stroke,
        );
    }

    // Draw value label immediately below the *visible body* of the symbol.
    // Each style has a different body extent beyond the pin stub, so the
    // offset is computed per-shape rather than a single constant.
    let body_extent = if id.contains("gnd") && !id.contains("earth") && !id.contains("gndref") {
        // 3 decreasing GND bars span ~1.2 mm from top bar to bottom bar.
        1.2
    } else if id.contains("gndref") {
        // Triangle height 1.27.
        1.27
    } else if id.contains("earth") {
        // Bar + hatch ~0.8 mm.
        0.9
    } else if id.contains("arrow") {
        // Triangle height 1.4.
        1.4
    } else if id.contains("circle") {
        // Circle diameter ~1.2 mm.
        1.2
    } else {
        // VCC / Bar: single bar has effectively zero extent beyond the pin.
        0.0
    };
    let label_y = (pin_len + body_extent + 0.25) * dir;
    // 10 pt — the canvas-wide default — matching Altium.
    let font_size_mm = crate::SCHEMATIC_TEXT_MM;
    let screen_font = transform.world_len(font_size_mm).abs();
    if screen_font >= 1.0 {
        let (tx, ty) = instance_transform(sym, &signex_types::schematic::Point::new(0.0, label_y));
        let sp = transform.to_screen_point(tx, ty);
        // Altium convention: the power-port label text is always drawn
        // upright regardless of symbol rotation. Only the label's POSITION
        // follows the rotation (via `instance_transform` above) so the text
        // sits just past the symbol body on the side away from the pin.
        let dx = tx - sym.position.x;
        let dy = ty - sym.position.y;
        let (align_x, align_y_v) = if dx.abs() > dy.abs() {
            // Label is horizontally offset (rotation 90° / 270°).
            let h = if dx > 0.0 {
                iced::alignment::Horizontal::Left
            } else {
                iced::alignment::Horizontal::Right
            };
            (h, iced::alignment::Vertical::Center)
        } else {
            // Label is vertically offset (rotation 0° / 180°).
            let v = if dy > 0.0 {
                iced::alignment::Vertical::Top
            } else {
                iced::alignment::Vertical::Bottom
            };
            (iced::alignment::Horizontal::Center, v)
        };
        frame.fill_text(canvas::Text {
            content: net,
            position: sp,
            color: label_color,
            size: iced::Pixels(screen_font),
            font: crate::canvas_font(),
            align_x: align_x.into(),
            align_y: align_y_v,
            ..canvas::Text::default()
        });
    }
}
