use super::*;

pub fn expand_char_escapes(text: &str) -> String {
    text.to_string()
}

pub fn escape_for_standard(text: &str) -> String {
    text.to_string()
}

pub fn draw_text_note_preview(
    frame: &mut canvas::Frame,
    note: &TextNote,
    transform: &ScreenTransform,
    color: Color,
) {
    let snapshot = RendererSnapshot {
        wires: Vec::new(),
        junctions: Vec::new(),
        arcs: Vec::new(),
        polygons: Vec::new(),
        labels: Vec::new(),
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts: vec![TextInput {
            content: note.text.clone(),
            position: [note.position.x as f32, note.position.y as f32],
            size_mm: note
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(color),
            bold: false,
            italic: false,
            rotation_rad: note.rotation.to_radians() as f32,
            h_align: note.justify_h,
            v_align: note.justify_v,
        }],
        overlays: OverlayInputs::default(),
        erc_markers: Vec::new(),
        wire_color_overrides: HashMap::new(),
    };

    draw_renderer_snapshot(
        frame,
        &snapshot,
        &ResolvedTheme::from_canvas_colors(signex_types::theme::canvas_colors(
            signex_types::theme::ThemeId::Signex,
        )),
        DirtyFlags::TEXT,
        transform,
    );
}
