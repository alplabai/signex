//! Schematic element rendering -- wires, symbols, labels, pins, etc.
//!
//! Each submodule handles rendering one schematic element type using
//! the Iced Canvas `Frame` API. All functions are pure: they take data,
//! a `ScreenTransform`, and colors, then draw onto the frame.

pub mod drawing;
pub mod junction;
pub mod label;
pub mod pin;
pub mod symbol;
pub mod text;
pub mod wire;

use iced::widget::canvas;
use iced::Rectangle;

use signex_types::schematic::{LabelType, SchematicSheet};
use signex_types::theme::CanvasColors;

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
// Main render entry point
// ---------------------------------------------------------------------------

/// Draw all elements of a schematic sheet onto the canvas frame.
///
/// Elements are rendered in z-order so that higher-layer items paint
/// on top of lower ones.
pub fn render_schematic(
    frame: &mut canvas::Frame,
    sheet: &SchematicSheet,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    _bounds: Rectangle,
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

    // Z=1: Drawing primitives (lines, rects, circles, arcs, polylines)
    for d in &sheet.drawings {
        drawing::draw_sch_drawing(frame, d, transform, body_color);
    }

    // Z=1b: Sheet rectangles
    for r in &sheet.rectangles {
        drawing::draw_sch_rectangle(frame, r, transform, body_color);
    }

    // Z=2: Wires
    for w in &sheet.wires {
        wire::draw_wire(frame, w, transform, wire_color);
    }

    // Z=3: Buses
    for b in &sheet.buses {
        wire::draw_bus(frame, b, transform, bus_color);
    }

    // Z=4: Bus entries
    for be in &sheet.bus_entries {
        wire::draw_bus_entry(frame, be, transform, bus_color);
    }

    // Z=5: Junctions
    for j in &sheet.junctions {
        junction::draw_junction(frame, j, transform, junction_color);
    }

    // Z=6: No-connect markers
    for nc in &sheet.no_connects {
        junction::draw_no_connect(frame, nc, transform, no_connect_color);
    }

    // Z=7-9: Labels (net, global, hierarchical, power)
    for lbl in &sheet.labels {
        let color = match lbl.label_type {
            LabelType::Net => to_iced(&colors.net_label),
            LabelType::Global => to_iced(&colors.global_label),
            LabelType::Hierarchical => to_iced(&colors.hier_label),
            LabelType::Power => to_iced(&colors.power),
        };
        label::draw_label(frame, lbl, transform, color);
    }

    // Z=10-11: Symbol bodies + pins
    for sym in &sheet.symbols {
        if let Some(lib_sym) = sheet.lib_symbols.get(&sym.lib_id) {
            symbol::draw_symbol(
                frame,
                sym,
                lib_sym,
                transform,
                body_color,
                body_fill_color,
                pin_color,
            );

            // Pins
            pin::draw_symbol_pins(
                frame,
                sym,
                lib_sym,
                transform,
                pin_color,
            );

            // Reference text
            if let Some(ref ref_text) = sym.ref_text {
                if !ref_text.hidden {
                    text::draw_text_prop(
                        frame,
                        &sym.reference,
                        ref_text,
                        transform,
                        reference_color,
                    );
                }
            }

            // Value text
            if let Some(ref val_text) = sym.val_text {
                if !val_text.hidden {
                    text::draw_text_prop(
                        frame,
                        &sym.value,
                        val_text,
                        transform,
                        value_color,
                    );
                }
            }
        }
    }

    // Z=11b: Child sheets (hierarchical sheets)
    for child in &sheet.child_sheets {
        drawing::draw_child_sheet(frame, child, transform, body_color, body_fill_color);
    }

    // Z=12: Text notes
    for tn in &sheet.text_notes {
        text::draw_text_note(frame, tn, transform, to_iced(&colors.body));
    }
}
