use signex_types::{
    rotation2d::normalize_angle_rad,
    schematic::{SchDrawing, SchematicSheet, SelectedItem, SelectedKind},
};

use crate::command::MirrorAxis;

use super::Engine;

impl Engine {
    pub(crate) fn contains_selected_item(&self, item: &SelectedItem) -> bool {
        if let Some(found) = self.contains_selected_sheet_item(item) {
            return found;
        }
        match item.kind {
            SelectedKind::Wire => self
                .document
                .wires
                .iter()
                .any(|wire| wire.uuid == item.uuid),
            SelectedKind::Bus => self.document.buses.iter().any(|bus| bus.uuid == item.uuid),
            SelectedKind::BusEntry => self
                .document
                .bus_entries
                .iter()
                .any(|bus_entry| bus_entry.uuid == item.uuid),
            SelectedKind::Label => self
                .document
                .labels
                .iter()
                .any(|label| label.uuid == item.uuid),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter()
                .any(|junction| junction.uuid == item.uuid),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter()
                .any(|no_connect| no_connect.uuid == item.uuid),
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter()
                .any(|symbol| symbol.uuid == item.uuid),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter()
                .any(|text_note| text_note.uuid == item.uuid),
            SelectedKind::Drawing => self
                .document
                .drawings
                .iter()
                .any(|d| drawing_uuid(d) == item.uuid),
            _ => false,
        }
    }

    /// `ChildSheet` / `SheetPin` share the same nested-ownership shape in
    /// both `contains_selected_item` and `remove_selected_item`, which
    /// pushed both matches past the house ~50-line cap. Split out so
    /// each caller stays a flat, single-purpose match. `None` means
    /// "not a sheet-owned kind — fall through to the caller's match".
    fn contains_selected_sheet_item(&self, item: &SelectedItem) -> Option<bool> {
        match item.kind {
            SelectedKind::SheetPin => Some(self.document.child_sheets.iter().any(|child_sheet| {
                child_sheet
                    .pins
                    .iter()
                    .any(|sheet_pin| sheet_pin.uuid == item.uuid)
            })),
            SelectedKind::ChildSheet => Some(
                self.document
                    .child_sheets
                    .iter()
                    .any(|child_sheet| child_sheet.uuid == item.uuid),
            ),
            _ => None,
        }
    }

    pub(super) fn remove_selected_item(&mut self, item: &SelectedItem) -> bool {
        match item.kind {
            SelectedKind::Wire => remove_by_uuid(&mut self.document.wires, item.uuid),
            SelectedKind::Bus => remove_by_uuid(&mut self.document.buses, item.uuid),
            SelectedKind::BusEntry => remove_by_uuid(&mut self.document.bus_entries, item.uuid),
            SelectedKind::Label => remove_by_uuid(&mut self.document.labels, item.uuid),
            SelectedKind::Junction => remove_by_uuid(&mut self.document.junctions, item.uuid),
            SelectedKind::NoConnect => remove_by_uuid(&mut self.document.no_connects, item.uuid),
            SelectedKind::Symbol => remove_by_uuid(&mut self.document.symbols, item.uuid),
            SelectedKind::TextNote => remove_by_uuid(&mut self.document.text_notes, item.uuid),
            SelectedKind::Drawing => {
                let before_len = self.document.drawings.len();
                self.document
                    .drawings
                    .retain(|d| drawing_uuid(d) != item.uuid);
                self.document.drawings.len() != before_len
            }
            // A child sheet owns its pins (`ChildSheet::pins`); removing the
            // sheet entry removes them with it in one shot — no separate
            // reconcile step needed, the whole owning document goes away.
            SelectedKind::ChildSheet => remove_by_uuid(&mut self.document.child_sheets, item.uuid),
            // A pin is owned by whichever child sheet holds it; delete just
            // that pin from its `pins` vec, leaving the sheet in place.
            //
            // This does NOT survive `reconcile_child_sheet_pins` (sheet.rs;
            // not yet wired into the app — #359). Reconcile derives a
            // sheet's pins from its currently-exposed hierarchical/global
            // ports, matched BY NAME: if a port with the deleted pin's name
            // is still exposed, the next reconcile recreates the pin from
            // scratch (fresh uuid, `auto_generated: true`) — the delete
            // doesn't stick. Only a pin whose backing port is also gone (or
            // that never had one, e.g. a hand-added `auto_generated: false`
            // pin with no matching port) stays deleted.
            SelectedKind::SheetPin => {
                for child_sheet in &mut self.document.child_sheets {
                    if remove_by_uuid(&mut child_sheet.pins, item.uuid) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub(super) fn move_selected_item(&mut self, item: &SelectedItem, dx: f64, dy: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    symbol.position.x += dx;
                    symbol.position.y += dy;
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += dx;
                        ref_text.position.y += dy;
                    }
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += dx;
                        val_text.position.y += dy;
                    }
                    true
                })
                .unwrap_or(false),
            SelectedKind::Wire => self
                .document
                .wires
                .iter_mut()
                .find(|wire| wire.uuid == item.uuid)
                .map(|wire| {
                    wire.start.x += dx;
                    wire.start.y += dy;
                    wire.end.x += dx;
                    wire.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Bus => self
                .document
                .buses
                .iter_mut()
                .find(|bus| bus.uuid == item.uuid)
                .map(|bus| {
                    bus.start.x += dx;
                    bus.start.y += dy;
                    bus.end.x += dx;
                    bus.end.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Label => self
                .document
                .labels
                .iter_mut()
                .find(|label| label.uuid == item.uuid)
                .map(|label| {
                    label.position.x += dx;
                    label.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::Junction => self
                .document
                .junctions
                .iter_mut()
                .find(|junction| junction.uuid == item.uuid)
                .map(|junction| {
                    junction.position.x += dx;
                    junction.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::NoConnect => self
                .document
                .no_connects
                .iter_mut()
                .find(|no_connect| no_connect.uuid == item.uuid)
                .map(|no_connect| {
                    no_connect.position.x += dx;
                    no_connect.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::TextNote => self
                .document
                .text_notes
                .iter_mut()
                .find(|text_note| text_note.uuid == item.uuid)
                .map(|text_note| {
                    text_note.position.x += dx;
                    text_note.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::ChildSheet => self
                .document
                .child_sheets
                .iter_mut()
                .find(|child_sheet| child_sheet.uuid == item.uuid)
                .map(|child_sheet| {
                    child_sheet.position.x += dx;
                    child_sheet.position.y += dy;
                    for sheet_pin in &mut child_sheet.pins {
                        sheet_pin.position.x += dx;
                        sheet_pin.position.y += dy;
                    }
                    true
                })
                .unwrap_or(false),
            SelectedKind::SheetPin => {
                for child_idx in 0..self.document.child_sheets.len() {
                    if let Some(pin_idx) = self.document.child_sheets[child_idx]
                        .pins
                        .iter()
                        .position(|sheet_pin| sheet_pin.uuid == item.uuid)
                    {
                        let (cx, cy, cw, ch) = {
                            let c = &self.document.child_sheets[child_idx];
                            (c.position.x, c.position.y, c.size.0, c.size.1)
                        };
                        let pin = &mut self.document.child_sheets[child_idx].pins[pin_idx];
                        super::sheet::lock_sheet_pin_to_child_edge(pin, dx, dy, cx, cy, cw, ch);
                        pin.user_moved = true;
                        return true;
                    }
                }
                false
            }
            SelectedKind::BusEntry => self
                .document
                .bus_entries
                .iter_mut()
                .find(|bus_entry| bus_entry.uuid == item.uuid)
                .map(|bus_entry| {
                    bus_entry.position.x += dx;
                    bus_entry.position.y += dy;
                    true
                })
                .unwrap_or(false),
            SelectedKind::SymbolRefField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.position.x += dx;
                        ref_text.position.y += dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::SymbolValField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.position.x += dx;
                        val_text.position.y += dy;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::Drawing => self
                .document
                .drawings
                .iter_mut()
                .find(|d| drawing_uuid(d) == item.uuid)
                .map(|d| {
                    match d {
                        SchDrawing::Line { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Rect { start, end, .. } => {
                            start.x += dx;
                            start.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Circle { center, .. } => {
                            center.x += dx;
                            center.y += dy;
                        }
                        SchDrawing::Arc {
                            start, mid, end, ..
                        } => {
                            start.x += dx;
                            start.y += dy;
                            mid.x += dx;
                            mid.y += dy;
                            end.x += dx;
                            end.y += dy;
                        }
                        SchDrawing::Polyline { points, .. } => {
                            for p in points {
                                p.x += dx;
                                p.y += dy;
                            }
                        }
                    }
                    true
                })
                .unwrap_or(false),
        }
    }

    pub(super) fn rotate_selected_item(&mut self, item: &SelectedItem, angle_degrees: f64) -> bool {
        match item.kind {
            SelectedKind::Symbol => {
                let lib_symbols = self.document.lib_symbols.clone();
                let document_snapshot = self.document.clone();
                self.document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == item.uuid)
                    .map(|symbol| {
                        symbol.rotation = normalize_degrees(symbol.rotation + angle_degrees);
                        if let Some(lib) = lib_symbols.get(&symbol.lib_id) {
                            autoplace_fields(symbol, lib, &document_snapshot);
                        }
                        true
                    })
                    .unwrap_or(false)
            }
            SelectedKind::SymbolRefField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    if let Some(ref mut ref_text) = symbol.ref_text {
                        ref_text.rotation = normalize_degrees(ref_text.rotation + angle_degrees);
                        // Manual field rotation marks the symbol as
                        // user-placed so future rotate / mirror operations
                        // never silently re-run the autoplacer over it.
                        symbol.fields_autoplaced = false;
                        symbol.fields_user_placed = true;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            SelectedKind::SymbolValField => self
                .document
                .symbols
                .iter_mut()
                .find(|symbol| symbol.uuid == item.uuid)
                .map(|symbol| {
                    if let Some(ref mut val_text) = symbol.val_text {
                        val_text.rotation = normalize_degrees(val_text.rotation + angle_degrees);
                        symbol.fields_autoplaced = false;
                        symbol.fields_user_placed = true;
                        true
                    } else {
                        false
                    }
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    pub(super) fn mirror_selected_item(&mut self, item: &SelectedItem, axis: MirrorAxis) -> bool {
        match item.kind {
            SelectedKind::Symbol => {
                let lib_symbols = self.document.lib_symbols.clone();
                let document_snapshot = self.document.clone();
                self.document
                    .symbols
                    .iter_mut()
                    .find(|symbol| symbol.uuid == item.uuid)
                    .map(|symbol| {
                        match axis {
                            MirrorAxis::Horizontal => symbol.mirror_y = !symbol.mirror_y,
                            MirrorAxis::Vertical => symbol.mirror_x = !symbol.mirror_x,
                        }
                        if let Some(lib) = lib_symbols.get(&symbol.lib_id) {
                            autoplace_fields(symbol, lib, &document_snapshot);
                        }
                        true
                    })
                    .unwrap_or(false)
            }
            _ => false,
        }
    }
}

fn normalize_degrees(angle_degrees: f64) -> f64 {
    normalize_angle_rad(angle_degrees.to_radians())
        .to_degrees()
        .rem_euclid(360.0)
}

mod autoplace;
pub(crate) use autoplace::needed_junction;
use autoplace::autoplace_fields;

// ---------------------------------------------------------------------------
// UUID-based collection helpers
// ---------------------------------------------------------------------------

/// `SchDrawing` doesn't implement `HasUuid` (its uuid lives inside each
/// enum variant, not on a common struct field) — shared by
/// `contains_selected_item`, `remove_selected_item` and
/// `move_selected_item` so the five-variant match lives in one place.
fn drawing_uuid(d: &SchDrawing) -> uuid::Uuid {
    match d {
        SchDrawing::Line { uuid, .. }
        | SchDrawing::Rect { uuid, .. }
        | SchDrawing::Circle { uuid, .. }
        | SchDrawing::Arc { uuid, .. }
        | SchDrawing::Polyline { uuid, .. } => *uuid,
    }
}

fn remove_by_uuid<T>(items: &mut Vec<T>, uuid: uuid::Uuid) -> bool
where
    T: HasUuid,
{
    let original_len = items.len();
    items.retain(|item| item.uuid() != uuid);
    original_len != items.len()
}

trait HasUuid {
    fn uuid(&self) -> uuid::Uuid;
}

macro_rules! impl_has_uuid {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl HasUuid for $ty {
                fn uuid(&self) -> uuid::Uuid {
                    self.uuid
                }
            }
        )+
    };
}

impl_has_uuid!(
    signex_types::schematic::Wire,
    signex_types::schematic::Bus,
    signex_types::schematic::BusEntry,
    signex_types::schematic::Label,
    signex_types::schematic::Junction,
    signex_types::schematic::NoConnect,
    signex_types::schematic::Symbol,
    signex_types::schematic::TextNote,
    signex_types::schematic::ChildSheet,
);
