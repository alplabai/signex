use super::*;

pub(in crate::gerber_viewer) fn inactive_layer_color(
    color: Color,
    active: bool,
    dim_inactive_layers: bool,
    inactive_layer_opacity: f32,
) -> Color {
    if !dim_inactive_layers || active {
        return color;
    }

    Color {
        a: color.a * inactive_layer_opacity.clamp(0.0, 1.0),
        ..color
    }
}

pub(in crate::gerber_viewer) fn forced_opacity_color(
    color: Color,
    forced_opacity_mode: bool,
    forced_opacity: f32,
) -> Color {
    if !forced_opacity_mode {
        return color;
    }

    Color {
        a: forced_opacity.clamp(0.0, 1.0),
        ..color
    }
}

pub(in crate::gerber_viewer) fn compare_layer_color(
    original: Color,
    visible_ordinal: usize,
    visible_layer_count: usize,
    compare_mode: bool,
    compare_palette: &[Color],
) -> Color {
    if !compare_mode || visible_layer_count < 2 || compare_palette.is_empty() {
        return original;
    }

    Color {
        a: 0.68,
        ..compare_palette[visible_ordinal % compare_palette.len()]
    }
}

#[cfg(test)]
pub(in crate::gerber_viewer) fn composite_compare_colors(top: Color, bottom: Color) -> Color {
    let alpha = top.a + bottom.a * (1.0 - top.a);
    Color {
        r: (top.r * top.a + bottom.r * bottom.a * (1.0 - top.a)) / alpha,
        g: (top.g * top.a + bottom.g * bottom.a * (1.0 - top.a)) / alpha,
        b: (top.b * top.a + bottom.b * bottom.a * (1.0 - top.a)) / alpha,
        a: alpha,
    }
}

pub(in crate::gerber_viewer) fn primitive_polarity_color(
    polarity: PrimitivePolarity,
    dark_color: Color,
    background: Color,
    negative_ghost_color: Color,
    ghost_negative_objects: bool,
) -> Color {
    match polarity {
        PrimitivePolarity::Dark => dark_color,
        PrimitivePolarity::Clear if ghost_negative_objects => negative_ghost_color,
        PrimitivePolarity::Clear => background,
    }
}

pub(super) struct LayerDrawOptions<'a> {
    pub(super) viewer_layer: &'a ViewerLayer,
    pub(super) layer_color: Color,
    pub(super) active: bool,
    pub(super) dim_inactive_layers: bool,
    pub(super) inactive_layer_opacity: f32,
    pub(super) scale: f32,
    pub(super) world_to_screen: &'a dyn Fn(signex_gerber::Point) -> Point,
    pub(super) background: Color,
    pub(super) highlighted_component: Option<&'a str>,
    pub(super) highlighted_net: Option<&'a str>,
    pub(super) highlighted_attribute: Option<&'a GerberAttributeValue>,
    pub(super) highlighted_d_code: Option<i32>,
    pub(super) selected_item: Option<GerberItemSelection>,
    pub(super) selected_items: &'a [GerberItemSelection],
    pub(super) layer_index: usize,
    pub(super) sketch_flashes: bool,
    pub(super) sketch_lines: bool,
    pub(super) sketch_polygons: bool,
    pub(super) ghost_negative_objects: bool,
    pub(super) negative_ghost_color: Color,
    pub(super) show_d_code_labels: bool,
    pub(super) d_code_color: Color,
    pub(super) zoom: f32,
}

pub(super) fn draw_layer(frame: &mut canvas::Frame, options: LayerDrawOptions<'_>) {
    let LayerDrawOptions {
        viewer_layer,
        layer_color,
        active,
        dim_inactive_layers,
        inactive_layer_opacity,
        scale,
        world_to_screen,
        background,
        highlighted_component,
        highlighted_net,
        highlighted_attribute,
        highlighted_d_code,
        selected_item,
        selected_items,
        layer_index,
        sketch_flashes,
        sketch_lines,
        sketch_polygons,
        ghost_negative_objects,
        negative_ghost_color,
        show_d_code_labels,
        d_code_color,
        zoom,
    } = options;
    for (primitive_index, primitive) in viewer_layer.layer.geometry.primitives.iter().enumerate() {
        let attributes = viewer_layer
            .layer
            .geometry
            .primitive_attributes
            .get(primitive_index);
        let component_color = component_highlight_color(
            inactive_layer_color(
                layer_color,
                active,
                dim_inactive_layers,
                inactive_layer_opacity,
            ),
            attributes,
            highlighted_component,
        );
        let net_color = net_highlight_color(component_color, attributes, highlighted_net);
        let attribute_color =
            attribute_highlight_color(net_color, attributes, highlighted_attribute);
        let dark_color = d_code_highlight_color(attribute_color, primitive, highlighted_d_code);
        let candidate = GerberItemSelection {
            layer_index,
            primitive_index,
        };
        let selected = selected_item == Some(candidate) || selected_items.contains(&candidate);
        let dark_color = selected_primitive_color(
            dark_color,
            selected_item,
            selected_items,
            layer_index,
            primitive_index,
        );
        let polarity_color = |polarity: PrimitivePolarity| {
            if selected {
                dark_color
            } else {
                primitive_polarity_color(
                    polarity,
                    dark_color,
                    background,
                    negative_ghost_color,
                    ghost_negative_objects,
                )
            }
        };
        match primitive {
            GerberPrimitive::Stroke {
                start,
                end,
                width,
                polarity,
                ..
            } => {
                paint_line_path(
                    frame,
                    &canvas::Path::line(world_to_screen(*start), world_to_screen(*end)),
                    polarity_color(*polarity),
                    background,
                    (*width as f32 * scale).max(0.8),
                    line_render_mode(sketch_lines),
                );
            }
            GerberPrimitive::Flash {
                position,
                aperture,
                polarity,
                ..
            } => {
                draw_flash(
                    frame,
                    world_to_screen(*position),
                    aperture,
                    scale,
                    polarity_color(*polarity),
                    flash_render_mode(sketch_flashes),
                );
            }
            GerberPrimitive::Region { points, polarity } => {
                if points.len() < 3 {
                    continue;
                }
                let path = canvas::Path::new(|builder| {
                    builder.move_to(world_to_screen(points[0]));
                    for point in &points[1..] {
                        builder.line_to(world_to_screen(*point));
                    }
                    builder.close();
                });
                paint_polygon_path(
                    frame,
                    &path,
                    polarity_color(*polarity),
                    polygon_render_mode(sketch_polygons),
                );
            }
            GerberPrimitive::DrillHit {
                position, diameter, ..
            } => {
                frame.fill(
                    &canvas::Path::circle(
                        world_to_screen(*position),
                        (*diameter as f32 * scale / 2.0).max(0.75),
                    ),
                    dark_color,
                );
            }
            GerberPrimitive::DrillSlot {
                start, end, width, ..
            } => {
                frame.stroke(
                    &canvas::Path::line(world_to_screen(*start), world_to_screen(*end)),
                    canvas::Stroke::default()
                        .with_color(dark_color)
                        .with_width((*width as f32 * scale).max(1.0)),
                );
            }
        }
        if d_code_labels_visible(show_d_code_labels, zoom)
            && let Some(label) = d_code_label(primitive)
        {
            let anchor = world_to_screen(label.anchor);
            frame.fill_text(canvas::Text {
                content: label.content,
                position: Point::new(anchor.x + 4.0, anchor.y - 4.0),
                color: d_code_color,
                size: iced::Pixels(11.0),
                align_x: iced::alignment::Horizontal::Left.into(),
                align_y: iced::alignment::Vertical::Bottom,
                ..canvas::Text::default()
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::gerber_viewer) struct DCodeLabel {
    pub(in crate::gerber_viewer) content: String,
    pub(in crate::gerber_viewer) anchor: signex_gerber::Point,
}

pub(in crate::gerber_viewer) fn d_code_labels_visible(show_d_code_labels: bool, zoom: f32) -> bool {
    show_d_code_labels && zoom >= 0.75
}

pub(in crate::gerber_viewer) fn d_code_label(primitive: &GerberPrimitive) -> Option<DCodeLabel> {
    match primitive {
        GerberPrimitive::Stroke {
            start,
            end,
            d_code: Some(d_code),
            ..
        } => Some(DCodeLabel {
            content: format!("D{d_code}"),
            anchor: signex_gerber::Point {
                x: (start.x + end.x) / 2.0,
                y: (start.y + end.y) / 2.0,
            },
        }),
        GerberPrimitive::Flash {
            position,
            d_code: Some(d_code),
            ..
        } => Some(DCodeLabel {
            content: format!("D{d_code}"),
            anchor: *position,
        }),
        _ => None,
    }
}

pub(in crate::gerber_viewer) fn draw_flash(
    frame: &mut canvas::Frame,
    center: Point,
    aperture: &ApertureShape,
    scale: f32,
    color: Color,
    render_mode: FlashRenderMode,
) {
    match aperture {
        ApertureShape::Circle { diameter } => {
            paint_flash_path(
                frame,
                &canvas::Path::circle(center, (*diameter as f32 * scale / 2.0).max(0.5)),
                color,
                render_mode,
            );
        }
        ApertureShape::Rectangle { width, height } | ApertureShape::Obround { width, height } => {
            let size = iced::Size::new(
                (*width as f32 * scale).max(1.0),
                (*height as f32 * scale).max(1.0),
            );
            paint_flash_path(
                frame,
                &canvas::Path::rectangle(
                    Point::new(center.x - size.width / 2.0, center.y - size.height / 2.0),
                    size,
                ),
                color,
                render_mode,
            );
        }
        ApertureShape::Polygon {
            diameter,
            vertices,
            rotation_degrees,
        } => {
            let count = usize::from((*vertices).max(3));
            let radius = *diameter as f32 * scale / 2.0;
            let rotation = (*rotation_degrees as f32).to_radians();
            let path = canvas::Path::new(|builder| {
                for index in 0..count {
                    let angle = rotation + std::f32::consts::TAU * index as f32 / count as f32;
                    let point = Point::new(
                        center.x + radius * angle.cos(),
                        center.y - radius * angle.sin(),
                    );
                    if index == 0 {
                        builder.move_to(point);
                    } else {
                        builder.line_to(point);
                    }
                }
                builder.close();
            });
            paint_flash_path(frame, &path, color, render_mode);
        }
        ApertureShape::Macro { .. } => {
            paint_flash_path(
                frame,
                &canvas::Path::circle(center, (0.075 * scale).max(1.5)),
                color,
                render_mode,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::gerber_viewer) enum FlashRenderMode {
    Filled,
    Outline,
}

pub(in crate::gerber_viewer) fn flash_render_mode(sketch_flashes: bool) -> FlashRenderMode {
    if sketch_flashes {
        FlashRenderMode::Outline
    } else {
        FlashRenderMode::Filled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::gerber_viewer) enum LineRenderMode {
    Filled,
    Outline,
}

pub(in crate::gerber_viewer) fn line_render_mode(sketch_lines: bool) -> LineRenderMode {
    if sketch_lines {
        LineRenderMode::Outline
    } else {
        LineRenderMode::Filled
    }
}

pub(in crate::gerber_viewer) fn line_stroke_widths(
    aperture_width: f32,
    render_mode: LineRenderMode,
) -> (f32, Option<f32>) {
    let aperture_width = aperture_width.max(0.8);
    let inner_width = match render_mode {
        LineRenderMode::Filled => None,
        LineRenderMode::Outline => {
            let inner_width = aperture_width - 2.0;
            (inner_width > 0.0).then_some(inner_width)
        }
    };
    (aperture_width, inner_width)
}

fn paint_line_path(
    frame: &mut canvas::Frame,
    path: &canvas::Path,
    color: Color,
    background: Color,
    aperture_width: f32,
    render_mode: LineRenderMode,
) {
    let (outer_width, inner_width) = line_stroke_widths(aperture_width, render_mode);
    frame.stroke(
        path,
        canvas::Stroke::default()
            .with_color(color)
            .with_width(outer_width)
            .with_line_cap(canvas::LineCap::Round),
    );
    if let Some(inner_width) = inner_width {
        frame.stroke(
            path,
            canvas::Stroke::default()
                .with_color(background)
                .with_width(inner_width)
                .with_line_cap(canvas::LineCap::Round),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::gerber_viewer) enum PolygonRenderMode {
    Filled,
    Outline,
}

pub(in crate::gerber_viewer) fn polygon_render_mode(sketch_polygons: bool) -> PolygonRenderMode {
    if sketch_polygons {
        PolygonRenderMode::Outline
    } else {
        PolygonRenderMode::Filled
    }
}

fn paint_polygon_path(
    frame: &mut canvas::Frame,
    path: &canvas::Path,
    color: Color,
    render_mode: PolygonRenderMode,
) {
    match render_mode {
        PolygonRenderMode::Filled => frame.fill(path, color),
        PolygonRenderMode::Outline => {
            frame.stroke(
                path,
                canvas::Stroke::default().with_color(color).with_width(1.0),
            );
        }
    }
}

fn paint_flash_path(
    frame: &mut canvas::Frame,
    path: &canvas::Path,
    color: Color,
    render_mode: FlashRenderMode,
) {
    match render_mode {
        FlashRenderMode::Filled => frame.fill(path, color),
        FlashRenderMode::Outline => {
            frame.stroke(
                path,
                canvas::Stroke::default().with_color(color).with_width(1.0),
            );
        }
    }
}
