//! Selection-overlay rendering — outlines around selected items.
//!
//! Painted into the *overlay* cache layer
//! ([`super::RenderLayers::overlay`]) so a selection / hover change
//! never invalidates the content cache. Each selected item is drawn
//! as a thin dashed rectangle around its world-space AABB; the AABB
//! is computed by re-using each primitive's `pub(crate) fn *_aabb`
//! helper so the outline always matches what the renderer drew.

use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use signex_types::schematic::{Aabb, SelectedKind};

use super::util::{iced_color, point_finite};
use super::{RenderContext, SchematicSnapshot, Viewport};

/// Padding (mm) added around each item's AABB before stroking the
/// selection outline. Keeps the dashed line a comfortable distance
/// off the body so it doesn't overlap stroked geometry.
pub const SELECTION_PADDING_MM: f64 = 0.762;

/// **Deprecated v0.12 alias** of [`render_selection_overlay`] using
/// the v0.11 4-argument signature `(frame, sheet, &[SelectedItem], &Viewport)`.
/// `selected` is consulted for emphasis even though the snapshot's
/// own selection slice is normally what the renderer reads.
#[deprecated(since = "0.12.0", note = "use render_selection_overlay")]
pub fn draw_selection_overlay(
    frame: &mut Frame,
    sheet: &signex_types::schematic::SchematicSheet,
    selected: &[signex_types::schematic::SelectedItem],
    viewport: &Viewport,
) {
    let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
    let snap = SchematicSnapshot::new(sheet, &theme).with_selection(selected);
    render_selection_overlay(frame, &snap, viewport);
}

/// Render the selection overlay for `snapshot.selection`.
///
/// Iterates the snapshot's selection list, looks up each item's
/// world-space AABB via the matching primitive's helper, and strokes
/// a dashed rectangle in the theme's `selection` colour.
pub fn render_selection_overlay(
    frame: &mut Frame,
    snapshot: &SchematicSnapshot<'_>,
    viewport: &Viewport,
) {
    if snapshot.selection.is_empty() {
        return;
    }
    let ctx = RenderContext::new(snapshot, viewport);
    let colour = iced_color(&ctx.theme().selection);
    let dash_segments: &[f32] = &[6.0, 3.0];
    let stroke = Stroke {
        line_dash: LineDash {
            offset: 0,
            segments: dash_segments,
        },
        ..Stroke::default().with_width(1.0).with_color(colour)
    };

    for item in snapshot.selection.iter().copied() {
        let Some(bbox) = item_aabb(snapshot, item) else {
            continue;
        };
        let padded = bbox.expand(SELECTION_PADDING_MM);
        let tl = viewport.world_to_screen(signex_types::schematic::Point::new(
            padded.min_x,
            padded.min_y,
        ));
        let br = viewport.world_to_screen(signex_types::schematic::Point::new(
            padded.max_x,
            padded.max_y,
        ));
        if !point_finite(tl) || !point_finite(br) {
            continue;
        }
        let path = Path::new(|builder| {
            builder.move_to(tl);
            builder.line_to(iced::Point::new(br.x, tl.y));
            builder.line_to(br);
            builder.line_to(iced::Point::new(tl.x, br.y));
            builder.close();
        });
        frame.stroke(&path, stroke);
    }
}

fn item_aabb(
    snapshot: &SchematicSnapshot<'_>,
    item: signex_types::schematic::SelectedItem,
) -> Option<Aabb> {
    use super::*;
    let sheet = snapshot.sheet;
    match item.kind {
        SelectedKind::Wire => sheet
            .wires
            .iter()
            .find(|w| w.uuid == item.uuid)
            .map(wire::wire_aabb),
        SelectedKind::Bus => sheet
            .buses
            .iter()
            .find(|b| b.uuid == item.uuid)
            .map(bus::bus_aabb),
        SelectedKind::BusEntry => sheet
            .bus_entries
            .iter()
            .find(|e| e.uuid == item.uuid)
            .map(bus_entry::bus_entry_aabb),
        SelectedKind::Junction => sheet
            .junctions
            .iter()
            .find(|j| j.uuid == item.uuid)
            .map(junction::junction_aabb),
        SelectedKind::NoConnect => sheet
            .no_connects
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(no_connect::no_connect_aabb),
        SelectedKind::Symbol => sheet.symbols.iter().find(|s| s.uuid == item.uuid).map(|s| {
            match snapshot.lib_symbol(&s.lib_id) {
                Some(lib) => symbol::symbol_aabb(s, lib),
                None => symbol::missing_symbol_aabb(s),
            }
        }),
        SelectedKind::SymbolRefField => sheet
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(symbol::ref_field_aabb),
        SelectedKind::SymbolValField => sheet
            .symbols
            .iter()
            .find(|s| s.uuid == item.uuid)
            .and_then(symbol::val_field_aabb),
        SelectedKind::ChildSheet => sheet
            .child_sheets
            .iter()
            .find(|cs| cs.uuid == item.uuid)
            .map(symbol::child_sheet_aabb),
        SelectedKind::SheetPin => sheet
            .child_sheets
            .iter()
            .find_map(|cs| cs.pins.iter().find(|p| p.uuid == item.uuid))
            .map(symbol::sheet_pin_aabb),
        SelectedKind::Label => sheet
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)
            .map(label::label_aabb),
        SelectedKind::TextNote => sheet
            .text_notes
            .iter()
            .find(|n| n.uuid == item.uuid)
            .map(text::text_note_aabb),
        SelectedKind::Drawing => sheet
            .drawings
            .iter()
            .find(|d| {
                use signex_types::schematic::SchDrawing;
                let uuid = match d {
                    SchDrawing::Line { uuid, .. }
                    | SchDrawing::Rect { uuid, .. }
                    | SchDrawing::Circle { uuid, .. }
                    | SchDrawing::Arc { uuid, .. }
                    | SchDrawing::Polyline { uuid, .. } => *uuid,
                };
                uuid == item.uuid
            })
            .map(drawing::drawing_aabb),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{Point, SchematicSheet, SelectedItem, Wire};
    use uuid::Uuid;

    fn sheet_with_wire() -> (SchematicSheet, SelectedItem) {
        let w = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stroke_width: 0.0,
        };
        let item = SelectedItem::new(w.uuid, SelectedKind::Wire);
        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: vec![w],
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: Default::default(),
            lib_symbols: Default::default(),
        };
        (sheet, item)
    }

    #[test]
    fn item_aabb_resolves_wire_uuid() {
        let (sheet, item) = sheet_with_wire();
        let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let snap = SchematicSnapshot::new(&sheet, &theme);
        let bbox = item_aabb(&snap, item);
        assert!(bbox.is_some());
        let b = bbox.unwrap();
        assert!(b.contains(5.0, 0.0));
    }

    #[test]
    fn item_aabb_returns_none_for_unknown_uuid() {
        // Edge case: stale selection from a previous edit — the item
        // is no longer in the sheet.
        let (sheet, _) = sheet_with_wire();
        let theme = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::Signex);
        let snap = SchematicSnapshot::new(&sheet, &theme);
        let stranger = SelectedItem::new(Uuid::new_v4(), SelectedKind::Wire);
        assert!(item_aabb(&snap, stranger).is_none());
    }
}
