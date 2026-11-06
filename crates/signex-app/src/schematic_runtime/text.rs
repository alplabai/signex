use super::*;

/// The text of a note in `color`, showing `content` (the sheet resolves
/// special strings first; the placement preview shows the raw text).
pub(super) fn note_text_input(
    note: &TextNote,
    content: String,
    color: Color,
    document_family: Option<&str>,
) -> TextInput {
    TextInput {
        content,
        position: [note.position.x as f32, note.position.y as f32],
        size_mm: signex_types::schematic::text_height_mm(note.font_size) as f32,
        color: to_rgba(color),
        bold: false,
        italic: false,
        family: resolve_family(note.font_family.as_deref(), document_family),
        rotation_rad: note.rotation.to_radians() as f32,
        h_align: note.justify_h,
        v_align: note.justify_v,
    }
}

pub fn draw_text_note_preview(
    frame: &mut canvas::Frame,
    note: &TextNote,
    transform: &ScreenTransform,
    color: Color,
    colors: &CanvasColors,
    document_family: Option<&str>,
) {
    let snapshot = RendererSnapshot {
        parameter_texts: vec![note_text_input(
            note,
            note.text.clone(),
            color,
            document_family,
        )],
        ..Default::default()
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::TEXT,
        transform,
    );
}
