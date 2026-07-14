use super::*;

pub fn draw_label_preview(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    stroke_color: Color,
    fill_color: Color,
) {
    let mut polygons = Vec::new();
    let mut labels = Vec::new();

    if matches!(
        label.label_type,
        LabelType::Global | LabelType::Hierarchical
    ) {
        polygons.push(super::label_marker_polygon(
            label,
            stroke_color,
            to_rgba(fill_color),
            transform,
        ));
        labels.push(TextInput {
            content: label.text.clone(),
            position: [label.position.x as f32, label.position.y as f32],
            size_mm: label
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(stroke_color),
            bold: false,
            italic: false,
            rotation_rad: label.rotation.to_radians() as f32,
            h_align: HAlign::Center,
            v_align: VAlign::Center,
        });
    } else {
        labels.push(TextInput {
            content: label.text.clone(),
            position: [label.position.x as f32, label.position.y as f32],
            size_mm: label
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(stroke_color),
            bold: false,
            italic: false,
            rotation_rad: label.rotation.to_radians() as f32,
            h_align: label.justify,
            v_align: label.justify_v,
        });
    }

    let snapshot = RendererSnapshot {
        wires: Vec::new(),
        junctions: Vec::new(),
        arcs: Vec::new(),
        polygons,
        labels,
        pin_texts: Vec::new(),
        reference_value_texts: Vec::new(),
        parameter_texts: Vec::new(),
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
        DirtyFlags::POLYGONS | DirtyFlags::TEXT,
        transform,
    );
}
