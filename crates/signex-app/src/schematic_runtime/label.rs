use super::*;

pub fn draw_label_preview(
    frame: &mut canvas::Frame,
    label: &Label,
    transform: &ScreenTransform,
    stroke_color: Color,
    fill_color: Color,
    colors: &CanvasColors,
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
        let (x, y) = flag_text_anchor(label);
        labels.push(TextInput {
            content: label.text.clone(),
            position: [x as f32, y as f32],
            size_mm: label
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(stroke_color),
            bold: false,
            italic: false,
            rotation_rad: label.rotation.to_radians() as f32,
            h_align: HAlign::Left,
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
        &ResolvedTheme::from_canvas_colors(*colors),
        DirtyFlags::POLYGONS | DirtyFlags::TEXT,
        transform,
    );
}

/// Global and hierarchical labels are drawn as a flag; net labels are bare text.
pub(super) fn is_flag(label: &Label) -> bool {
    matches!(
        label.label_type,
        LabelType::Global | LabelType::Hierarchical
    )
}

/// Flag geometry in the label's local frame (anchor at the origin, +x the
/// direction the label extends): returns `(vertices, text_inset)`. The arrow
/// tip sits at `-point` so the wire end lands inside the tip; the body runs
/// from `0` to `w`, and text starts `text_inset` into it.
fn flag_local(label: &Label) -> ([(f64, f64); 5], f64) {
    let size_mm = label
        .font_size
        .max(signex_types::schematic::SCHEMATIC_TEXT_MM);
    let em_mm = size_mm / 0.72;
    let glyph_w = (label.text.chars().count().max(1) as f64) * (em_mm * 0.58);
    let half_h = em_mm * 0.62;
    let point = em_mm * 0.52;
    let pad = em_mm * 0.325;
    let w = glyph_w + pad * 2.0;
    (
        [
            (-point, 0.0),
            (0.0, -half_h),
            (w, -half_h),
            (w, half_h),
            (0.0, half_h),
        ],
        pad,
    )
}

fn to_world(label: &Label, (x, y): (f64, f64)) -> (f64, f64) {
    let (sin, cos) = label.rotation.to_radians().sin_cos();
    (
        label.position.x + x * cos - y * sin,
        label.position.y + x * sin + y * cos,
    )
}

/// World-space flag outline; the single source for draw and hit-test.
pub(super) fn marker_vertices(label: &Label) -> Vec<(f64, f64)> {
    flag_local(label)
        .0
        .iter()
        .map(|&p| to_world(label, p))
        .collect()
}

/// World-space start of the flag text (left-aligned, vertically centred),
/// inside the body so the text runs away from the connection point.
pub(super) fn flag_text_anchor(label: &Label) -> (f64, f64) {
    to_world(label, (flag_local(label).1, 0.0))
}
