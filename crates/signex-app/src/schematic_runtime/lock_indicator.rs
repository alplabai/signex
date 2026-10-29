//! A small padlock above every locked symbol (#1250), wire, label, text note
//! and drawing (#1573), drawn in the selection layer so it follows the
//! camera and never enters the scene cache. The colour is the theme's
//! `value` token.

use super::*;

/// Whether anything of `snapshot` is locked, i.e. the layer has a padlock
/// to draw even with nothing selected.
pub fn has_locked_item(snapshot: &SchematicRenderSnapshot) -> bool {
    !locked_items(snapshot).is_empty()
}

/// Every locked object of `snapshot`, as the selection items its bounds are
/// read through.
fn locked_items(snapshot: &SchematicRenderSnapshot) -> Vec<SelectedItem> {
    let item = |uuid, kind| SelectedItem::new(uuid, kind);
    let symbols = snapshot
        .symbols
        .iter()
        .filter(|s| s.locked)
        .map(|s| item(s.uuid, SelectedKind::Symbol));
    let wires = snapshot
        .wires
        .iter()
        .filter(|w| w.locked)
        .map(|w| item(w.uuid, SelectedKind::Wire));
    let labels = snapshot
        .labels
        .iter()
        .filter(|l| l.locked)
        .map(|l| item(l.uuid, SelectedKind::Label));
    let notes = snapshot
        .text_notes
        .iter()
        .filter(|n| n.locked)
        .map(|n| item(n.uuid, SelectedKind::TextNote));
    let drawings = snapshot
        .drawings
        .iter()
        .filter(|d| d.is_locked())
        .map(|d| item(d.uuid(), SelectedKind::Drawing));
    symbols
        .chain(wires)
        .chain(labels)
        .chain(notes)
        .chain(drawings)
        .collect()
}

pub fn draw_lock_indicators(
    frame: &mut canvas::Frame,
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    designators: &Designators,
) {
    let locked = locked_items(snapshot);
    if locked.is_empty() {
        return;
    }
    let color = to_rgba(to_iced(&colors.value));
    let px = |n: f32| screen_px_to_world_mm(n, transform.scale) as f32;
    let mut overlays = OverlayInputs::default();

    for bbox in selected_aabbs(snapshot, &locked, designators)
        .into_iter()
        .flatten()
    {
        // Top-right corner of the symbol; world y grows downward.
        let (right, top) = (bbox.max_x as f32, bbox.min_y as f32);
        let (left, base) = (right - px(7.0), top - px(5.0));
        overlays.ghost_polygons.push(OverlayPolygonInput {
            vertices: vec![[left, base], [right, base], [right, top], [left, top]],
            fill_color: color,
            stroke_color: None,
            stroke_width_mm: 0.0,
        });
        let (a, b, rim) = (left + px(1.5), right - px(1.5), base - px(3.0));
        for (p0, p1) in [
            ([a, base], [a, rim]),
            ([a, rim], [b, rim]),
            ([b, rim], [b, base]),
        ] {
            overlays.preview_lines.push(OverlayLineInput {
                p0,
                p1,
                width_mm: px(1.2),
                color,
            });
        }
    }

    let snapshot = RendererSnapshot {
        wires: Vec::new(),
        junctions: Vec::new(),
        arcs: Vec::new(),
        polygons: Vec::new(),
        labels: Vec::new(),
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts: Vec::new(),
        overlays,
        erc_markers: Vec::new(),
        drawings: Vec::new(),
        drawings_front: Vec::new(),
        wire_color_overrides: HashMap::new(),
        wire_line_styles: HashMap::new(),
    };
    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::OVERLAY,
        transform,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::schematic::{HAlign, Label, LabelType, SchDrawing, TextNote, VAlign, Wire};

    fn label(locked: bool) -> Label {
        Label {
            uuid: uuid::Uuid::nil(),
            text: "N".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: None,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked,
            unknown_columns: Default::default(),
        }
    }

    fn note(locked: bool) -> TextNote {
        TextNote {
            uuid: uuid::Uuid::nil(),
            text: "n".into(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            locked,
        }
    }

    fn sheet() -> SchematicRenderSnapshot {
        SchematicSheet::default()
    }

    #[test]
    fn nothing_locked_draws_no_padlock() {
        let mut sheet = sheet();
        sheet.wires.push(Wire::default());
        sheet.labels.push(label(false));
        sheet.text_notes.push(note(false));
        assert!(!has_locked_item(&sheet));
    }

    #[test]
    fn a_locked_wire_label_note_or_drawing_gets_a_padlock() {
        let wire = Wire {
            locked: true,
            ..Default::default()
        };
        let mut with_wire = sheet();
        with_wire.wires.push(wire);
        assert!(has_locked_item(&with_wire));

        let mut with_label = sheet();
        with_label.labels.push(label(true));
        assert!(has_locked_item(&with_label));

        let mut with_note = sheet();
        with_note.text_notes.push(note(true));
        assert!(has_locked_item(&with_note));

        let mut with_drawing = sheet();
        with_drawing.drawings.push(SchDrawing::Line {
            uuid: uuid::Uuid::nil(),
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
            width: 0.1,
            stroke_color: None,
            locked: true,
        });
        assert!(has_locked_item(&with_drawing));
        assert_eq!(locked_items(&with_drawing).len(), 1);
    }
}
