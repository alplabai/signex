//! Stock `.snxsym` authoring: shared helpers plus one module per
//! symbol family (`passive` today; connectors and logic add siblings).
//!
//! Every shape is drawn from first principles in the IEC 60617 /
//! IEEE 315 style. All geometry is symbol-local mm, y-up.
//!
//! Adding a symbol: append a pinned row to `ids::STOCK_SYMBOL_IDS`,
//! build it with [`symbol`] + the pin/graphic helpers below, and list
//! the builder in its family's `all()`. Chain a new family into
//! [`build_all`].

pub mod connector;
pub mod logic;
pub mod passive;

use std::error::Error;

use signex_library::primitive::symbol::{
    PinDirection, PinOrientation, Symbol, SymbolFile, SymbolGraphic, SymbolGraphicKind, SymbolPin,
};

use super::ids::symbol_ids;
use super::stock_timestamp;

/// Schematic snap grid in mm (50 mil), the default canvas grid. Every
/// pin connection point must sit on a multiple of this so a wire
/// started at the pin snaps to it.
pub const GRID_MM: f64 = 1.27;

/// Default pin stub length: two grid steps.
pub const PIN_LENGTH_MM: f64 = 2.54;

/// Stroke width for body outlines, mm.
pub const STROKE_MM: f64 = 0.254;

/// Round to 1e-4 mm so computed coordinates serialise to short,
/// stable decimals.
pub fn round4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

/// Snap a coordinate to the nearest grid point.
pub fn snap(v: f64) -> f64 {
    round4((v / GRID_MM).round() * GRID_MM)
}

/// A pin whose connection point is `(x, y)`; `orientation` is the
/// direction the stub extends from that point toward the body. The
/// point is snapped to the grid.
pub fn pin(
    number: &str,
    name: &str,
    electrical: PinDirection,
    x: f64,
    y: f64,
    orientation: PinOrientation,
) -> SymbolPin {
    let mut p = SymbolPin::new(number, name);
    p.electrical = electrical;
    p.position = [snap(x), snap(y)];
    p.orientation = orientation;
    p.length = PIN_LENGTH_MM;
    p
}

/// Hide the pin name text (two-terminal parts draw only the number).
pub fn name_hidden(mut p: SymbolPin) -> SymbolPin {
    p.name_visible = false;
    p
}

/// Set the pin stub length.
pub fn with_length(mut p: SymbolPin, length: f64) -> SymbolPin {
    p.length = length;
    p
}

/// Hide both the number and the name text.
pub fn labels_hidden(mut p: SymbolPin) -> SymbolPin {
    p.name_visible = false;
    p.designator_visible = false;
    p
}

fn graphic(kind: SymbolGraphicKind, fill: Option<[u8; 4]>) -> SymbolGraphic {
    SymbolGraphic {
        kind,
        stroke_width: STROKE_MM,
        fill,
        part_number: 0,
    }
}

/// Straight line segment.
pub fn line(from: [f64; 2], to: [f64; 2]) -> SymbolGraphic {
    graphic(SymbolGraphicKind::Line { from, to }, None)
}

/// Outlined axis-aligned rectangle given two opposite corners.
pub fn rect(from: [f64; 2], to: [f64; 2]) -> SymbolGraphic {
    graphic(SymbolGraphicKind::Rectangle { from, to }, None)
}

/// Outlined circle.
pub fn circle(center: [f64; 2], radius: f64) -> SymbolGraphic {
    graphic(SymbolGraphicKind::Circle { center, radius }, None)
}

/// Counter-clockwise arc from `start_deg` to `end_deg`.
pub fn arc(center: [f64; 2], radius: f64, start_deg: f64, end_deg: f64) -> SymbolGraphic {
    graphic(
        SymbolGraphicKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        },
        None,
    )
}

/// Outlined closed polygon.
pub fn polygon(vertices: Vec<[f64; 2]>) -> SymbolGraphic {
    graphic(SymbolGraphicKind::Polygon { vertices }, None)
}

/// Solid-filled closed polygon (arrow heads, power-port triangles).
pub fn filled_polygon(vertices: Vec<[f64; 2]>) -> SymbolGraphic {
    graphic(
        SymbolGraphicKind::Polygon { vertices },
        Some([0, 0, 0, 255]),
    )
}

/// Solid triangular arrow head with its apex at `tip`, pointing away
/// from `from` (a point back along the shaft).
pub fn arrow_head(tip: [f64; 2], from: [f64; 2], len: f64, half_width: f64) -> SymbolGraphic {
    let (dx, dy) = (tip[0] - from[0], tip[1] - from[1]);
    let norm = dx.hypot(dy);
    let (ux, uy) = (dx / norm, dy / norm);
    let base = [tip[0] - ux * len, tip[1] - uy * len];
    let (px, py) = (-uy * half_width, ux * half_width);
    filled_polygon(vec![
        [round4(tip[0]), round4(tip[1])],
        [round4(base[0] + px), round4(base[1] + py)],
        [round4(base[0] - px), round4(base[1] - py)],
    ])
}

/// Assemble a symbol shell. Identity is attached later by
/// [`render_all`] from the pinned id table, keyed by `name`.
pub fn symbol(
    name: &str,
    designator: &str,
    description: &str,
    pins: Vec<SymbolPin>,
    graphics: Vec<SymbolGraphic>,
) -> Symbol {
    let mut s = Symbol::empty(name);
    s.designator = designator.to_string();
    s.description = description.to_string();
    s.pins = pins;
    s.graphics = graphics;
    s
}

/// One rendered stock symbol file.
pub struct RenderedSymbol {
    pub file_name: String,
    pub symbol_name: String,
    pub pin_count: usize,
    pub designator: String,
    pub text: String,
}

/// Every stock symbol, family by family.
pub fn build_all() -> Vec<Symbol> {
    let mut all = passive::all();
    all.extend(connector::all());
    all.extend(logic::all());
    all
}

/// Render every stock symbol to `symbols/<name>.snxsym` text. Pure
/// and deterministic: ids come from the pinned table, timestamps from
/// `stock_timestamp`.
pub fn render_all() -> Result<Vec<RenderedSymbol>, Box<dyn Error>> {
    let stamp = stock_timestamp();
    let mut out = Vec::new();
    for mut sym in build_all() {
        let (file_uuid, symbol_uuid) = symbol_ids(&sym.name);
        sym.uuid = symbol_uuid;
        sym.created = stamp;
        sym.updated = stamp;
        let file_name = format!("{}.snxsym", sym.name);
        let symbol_name = sym.name.clone();
        let pin_count = sym.pins.len();
        let designator = sym.designator.clone();
        let mut file = SymbolFile::from_symbol(sym);
        file.file_uuid = file_uuid;
        file.created = stamp;
        file.updated = stamp;
        out.push(RenderedSymbol {
            file_name,
            symbol_name,
            pin_count,
            designator,
            text: file.to_toml_string()?,
        });
    }
    Ok(out)
}
