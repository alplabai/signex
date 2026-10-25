use super::*;

pub fn build_renderer_snapshot(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wire_color_overrides: Option<&HashMap<uuid::Uuid, ThemeColor>>,
) -> RendererSnapshot {
    let mut wires = Vec::new();
    let mut junctions = Vec::with_capacity(snapshot.junctions.len());
    let mut arcs = Vec::new();
    let mut polygons = Vec::new();
    let mut labels = Vec::new();
    let mut reference_value_texts = Vec::new();
    let mut parameter_texts = Vec::new();

    for wire in &snapshot.wires {
        let p0 = transform.world_to_screen((wire.start.x, wire.start.y));
        let p1 = transform.world_to_screen((wire.end.x, wire.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }

        let base_color = wire_color_overrides
            .and_then(|map| map.get(&wire.uuid))
            .map(to_iced)
            .unwrap_or_else(|| to_iced(&colors.wire));
        let color = focus_color(base_color, focus_set, wire.uuid);
        wires.push(WireInput {
            id: renderer_id(wire.uuid),
            p0: [wire.start.x as f32, wire.start.y as f32],
            p1: [wire.end.x as f32, wire.end.y as f32],
            width_mm: wire
                .stroke_width
                .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                as f32,
            explicit_color: Some(to_rgba(color)),
        });
    }

    for bus in &snapshot.buses {
        let p0 = transform.world_to_screen((bus.start.x, bus.start.y));
        let p1 = transform.world_to_screen((bus.end.x, bus.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }

        wires.push(WireInput {
            id: renderer_id(bus.uuid),
            p0: [bus.start.x as f32, bus.start.y as f32],
            p1: [bus.end.x as f32, bus.end.y as f32],
            width_mm: signex_types::schematic::SCHEMATIC_RENDER_BUS_STROKE_MM as f32,
            explicit_color: Some(to_rgba(focus_color(
                to_iced(&colors.bus),
                focus_set,
                bus.uuid,
            ))),
        });
    }

    for no_connect in &snapshot.no_connects {
        let center = transform.world_to_screen((no_connect.position.x, no_connect.position.y));
        if !point_visible(center, bounds, 10.0) {
            continue;
        }

        let color = focus_color(to_iced(&colors.body), focus_set, no_connect.uuid);
        let len_mm = signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM.max(
            screen_px_to_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_MIN_HALF_LEN_PX,
                transform.scale,
            ),
        );
        let width_mm = stroke_world_mm(
            signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_STROKE_PX,
            transform.scale,
        );
        let (cx, cy) = (no_connect.position.x as f32, no_connect.position.y as f32);
        let len = len_mm as f32;
        wires.push(WireInput {
            id: renderer_id(no_connect.uuid),
            p0: [cx - len, cy - len],
            p1: [cx + len, cy + len],
            width_mm,
            explicit_color: Some(to_rgba(color)),
        });
        wires.push(WireInput {
            id: renderer_id(no_connect.uuid).saturating_add(1),
            p0: [cx - len, cy + len],
            p1: [cx + len, cy - len],
            width_mm,
            explicit_color: Some(to_rgba(color)),
        });
    }

    // No-ERC directive: a diamond outline in the `no_connect` theme token,
    // so it never reads as the no-connect X (which uses the body colour).
    for directive in &snapshot.no_erc_directives {
        let center = transform.world_to_screen((directive.position.x, directive.position.y));
        if !point_visible(center, bounds, 10.0) {
            continue;
        }

        let color = focus_color(to_iced(&colors.no_connect), focus_set, directive.uuid);
        let half = signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM.max(
            screen_px_to_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_MIN_HALF_LEN_PX,
                transform.scale,
            ),
        ) as f32;
        let width_mm = stroke_world_mm(
            signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_STROKE_PX,
            transform.scale,
        );
        let (cx, cy) = (directive.position.x as f32, directive.position.y as f32);
        let corners = [
            [cx, cy - half],
            [cx + half, cy],
            [cx, cy + half],
            [cx - half, cy],
        ];
        for (i, p0) in corners.iter().enumerate() {
            wires.push(WireInput {
                id: renderer_id(directive.uuid).saturating_add(i as u64),
                p0: *p0,
                p1: corners[(i + 1) % corners.len()],
                width_mm,
                explicit_color: Some(to_rgba(color)),
            });
        }
    }

    for junction in &snapshot.junctions {
        let center = transform.world_to_screen((junction.position.x, junction.position.y));
        if !point_visible(center, bounds, 6.0) {
            continue;
        }

        junctions.push(JunctionInput {
            center: [junction.position.x as f32, junction.position.y as f32],
            radius_mm: (junction.diameter * 0.5)
                .max(signex_types::schematic::SCHEMATIC_RENDER_JUNCTION_MIN_RADIUS_MM)
                as f32,
            color: to_rgba(focus_color(
                to_iced(&colors.junction),
                focus_set,
                junction.uuid,
            )),
        });
    }

    for symbol in &snapshot.symbols {
        let bbox = symbol_body_aabb(symbol);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let stroke_color = focus_color(to_iced(&colors.body), focus_set, symbol.uuid);
        let fill_color = focus_color(to_iced(&colors.body_fill), focus_set, symbol.uuid);
        polygons.push(PolygonInput {
            vertices: vec![
                [bbox.min_x as f32, bbox.min_y as f32],
                [bbox.max_x as f32, bbox.min_y as f32],
                [bbox.max_x as f32, bbox.max_y as f32],
                [bbox.min_x as f32, bbox.max_y as f32],
            ],
            fill_color: to_rgba(fill_color),
            stroke_color: Some(to_rgba(stroke_color)),
            stroke_width_mm: stroke_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_SYMBOL_BODY_STROKE_PX,
                transform.scale,
            ),
        });

        let value_color = focus_color(to_iced(&colors.value), focus_set, symbol.uuid);
        if !symbol.reference.is_empty()
            && let Some(text) = field_text_input(
                symbol,
                &symbol.reference,
                symbol.ref_text.as_ref(),
                FieldSlot::Reference,
                to_rgba(stroke_color),
            )
        {
            reference_value_texts.push(text);
        }
        if !symbol.value.is_empty()
            && let Some(text) = field_text_input(
                symbol,
                &symbol.value,
                symbol.val_text.as_ref(),
                FieldSlot::Value,
                to_rgba(value_color),
            )
        {
            reference_value_texts.push(text);
        }
    }

    for sheet in &snapshot.child_sheets {
        let x0 = sheet.position.x;
        let y0 = sheet.position.y;
        let x1 = sheet.position.x + sheet.size.0;
        let y1 = sheet.position.y + sheet.size.1;
        let min = transform.world_to_screen((x0, y0));
        let max = transform.world_to_screen((x1, y1));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let color = focus_color(to_iced(&colors.global_label), focus_set, sheet.uuid);
        let min_x = x0.min(x1) as f32;
        let min_y = y0.min(y1) as f32;
        let max_x = x0.max(x1) as f32;
        let max_y = y0.max(y1) as f32;
        polygons.push(PolygonInput {
            vertices: vec![
                [min_x, min_y],
                [max_x, min_y],
                [max_x, max_y],
                [min_x, max_y],
            ],
            fill_color: [0.0, 0.0, 0.0, 0.0],
            stroke_color: Some(to_rgba(color)),
            stroke_width_mm: stroke_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_CHILD_SHEET_STROKE_PX,
                transform.scale,
            ),
        });

        parameter_texts.push(TextInput {
            content: sheet.name.clone(),
            position: [
                min_x + screen_px_to_world_mm(6.0, transform.scale) as f32,
                min_y + screen_px_to_world_mm(6.0, transform.scale) as f32,
            ],
            size_mm: 1.05,
            color: to_rgba(color),
            bold: false,
            italic: false,
            rotation_rad: 0.0,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
        });

        for pin in &sheet.pins {
            junctions.push(JunctionInput {
                center: [pin.position.x as f32, pin.position.y as f32],
                radius_mm: screen_px_to_world_mm(
                    signex_types::schematic::SCHEMATIC_RENDER_CHILD_SHEET_PIN_RADIUS_PX,
                    transform.scale,
                ) as f32,
                color: to_rgba(Color { a: 0.3, ..color }),
            });
        }
    }

    for drawing in &snapshot.drawings {
        let uuid = match drawing {
            SchDrawing::Line { uuid, .. }
            | SchDrawing::Rect { uuid, .. }
            | SchDrawing::Circle { uuid, .. }
            | SchDrawing::Arc { uuid, .. }
            | SchDrawing::Polyline { uuid, .. } => *uuid,
        };
        let bbox = drawing_aabb(drawing);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let base_color = focus_color(to_iced(&colors.body), focus_set, uuid);
        match drawing {
            SchDrawing::Line {
                start,
                end,
                width,
                stroke_color,
                ..
            } => {
                wires.push(WireInput {
                    id: renderer_id(uuid),
                    p0: [start.x as f32, start.y as f32],
                    p1: [end.x as f32, end.y as f32],
                    width_mm: width.max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                        as f32,
                    explicit_color: Some(to_rgba(resolve_stroke_color(stroke_color, base_color))),
                });
            }
            SchDrawing::Rect {
                start,
                end,
                width,
                fill,
                stroke_color,
                ..
            } => {
                polygons.push(PolygonInput {
                    vertices: vec![
                        [start.x as f32, start.y as f32],
                        [end.x as f32, start.y as f32],
                        [end.x as f32, end.y as f32],
                        [start.x as f32, end.y as f32],
                    ],
                    fill_color: fill_color_for(*fill, stroke_color, colors)
                        .map(to_rgba)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    stroke_color: Some(to_rgba(resolve_stroke_color(stroke_color, base_color))),
                    stroke_width_mm: width
                        .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                        as f32,
                });
            }
            SchDrawing::Circle {
                center,
                radius,
                width,
                fill,
                stroke_color,
                ..
            } => {
                polygons.push(PolygonInput {
                    vertices: circle_vertices(
                        [center.x, center.y],
                        radius.max(screen_px_to_world_mm(
                            signex_types::schematic::SCHEMATIC_RENDER_DRAWING_MIN_CIRCLE_RADIUS_PX,
                            transform.scale,
                        )) as f32,
                        40,
                    ),
                    fill_color: fill_color_for(*fill, stroke_color, colors)
                        .map(to_rgba)
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]),
                    stroke_color: Some(to_rgba(resolve_stroke_color(stroke_color, base_color))),
                    stroke_width_mm: width
                        .max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                        as f32,
                });
            }
            SchDrawing::Arc {
                start,
                mid,
                end,
                width,
                stroke_color,
                ..
            } => {
                if let Some((cx, cy, r)) = circumcircle(*start, *mid, *end) {
                    let a0 = (start.y - cy).atan2(start.x - cx);
                    let am = (mid.y - cy).atan2(mid.x - cx);
                    let a1 = (end.y - cy).atan2(end.x - cx);
                    let (start_angle, end_angle) = if arc_sweeps_through_mid(a0, am, a1) {
                        (a0, a1)
                    } else {
                        (a1, a0)
                    };
                    arcs.push(ArcInput {
                        center: [cx as f32, cy as f32],
                        radius_mm: r.max(screen_px_to_world_mm(
                            signex_types::schematic::SCHEMATIC_RENDER_DRAWING_MIN_ARC_RADIUS_PX,
                            transform.scale,
                        )) as f32,
                        start_angle_rad: start_angle as f32,
                        end_angle_rad: end_angle as f32,
                        width_mm: width.max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM)
                            as f32,
                        color: to_rgba(resolve_stroke_color(stroke_color, base_color)),
                    });
                } else {
                    let stroke_color = to_rgba(resolve_stroke_color(stroke_color, base_color));
                    let width_mm =
                        width.max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM) as f32;
                    wires.push(WireInput {
                        id: renderer_id(uuid),
                        p0: [start.x as f32, start.y as f32],
                        p1: [mid.x as f32, mid.y as f32],
                        width_mm,
                        explicit_color: Some(stroke_color),
                    });
                    wires.push(WireInput {
                        id: renderer_id(uuid).saturating_add(1),
                        p0: [mid.x as f32, mid.y as f32],
                        p1: [end.x as f32, end.y as f32],
                        width_mm,
                        explicit_color: Some(stroke_color),
                    });
                }
            }
            SchDrawing::Polyline {
                points,
                width,
                fill,
                stroke_color,
                ..
            } => {
                if points.len() < 2 {
                    continue;
                }

                let stroke = to_rgba(resolve_stroke_color(stroke_color, base_color));
                let width_mm =
                    width.max(signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM) as f32;
                if matches!(fill, FillType::None) {
                    for idx in 1..points.len() {
                        let p0 = points[idx - 1];
                        let p1 = points[idx];
                        wires.push(WireInput {
                            id: renderer_id(uuid).saturating_add(idx as u64),
                            p0: [p0.x as f32, p0.y as f32],
                            p1: [p1.x as f32, p1.y as f32],
                            width_mm,
                            explicit_color: Some(stroke),
                        });
                    }
                } else {
                    polygons.push(PolygonInput {
                        vertices: points
                            .iter()
                            .map(|point| [point.x as f32, point.y as f32])
                            .collect(),
                        fill_color: fill_color_for(*fill, stroke_color, colors)
                            .map(to_rgba)
                            .unwrap_or([0.0, 0.0, 0.0, 0.0]),
                        stroke_color: Some(stroke),
                        stroke_width_mm: width_mm,
                    });
                }
            }
        }
    }

    for label in &snapshot.labels {
        let screen = transform.world_to_screen((label.position.x, label.position.y));
        if !text_anchor_visible(screen, bounds, 22.0, &label_aabb(label), transform.scale) {
            continue;
        }

        let color = focus_color(label_color(label, colors), focus_set, label.uuid);
        if matches!(
            label.label_type,
            LabelType::Global | LabelType::Hierarchical
        ) {
            polygons.push(label_marker_polygon(
                label,
                color,
                [0.0, 0.0, 0.0, 0.0],
                transform,
            ));
            let (tx, ty) = label::flag_text_anchor(label);
            labels.push(TextInput {
                content: label.text.clone(),
                position: [tx as f32, ty as f32],
                size_mm: label
                    .font_size
                    .max(signex_types::schematic::SCHEMATIC_TEXT_MM)
                    as f32,
                color: to_rgba(color),
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
                    .max(signex_types::schematic::SCHEMATIC_TEXT_MM)
                    as f32,
                color: to_rgba(color),
                bold: false,
                italic: false,
                rotation_rad: label.rotation.to_radians() as f32,
                h_align: label.justify,
                v_align: label.justify_v,
            });
        }
    }

    for note in &snapshot.text_notes {
        let pos = transform.world_to_screen((note.position.x, note.position.y));
        if !text_anchor_visible(pos, bounds, 28.0, &note_aabb(note), transform.scale) {
            continue;
        }

        parameter_texts.push(TextInput {
            content: note.text.clone(),
            position: [note.position.x as f32, note.position.y as f32],
            size_mm: note
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(focus_color(to_iced(&colors.value), focus_set, note.uuid)),
            bold: false,
            italic: false,
            rotation_rad: note.rotation.to_radians() as f32,
            h_align: note.justify_h,
            v_align: note.justify_v,
        });
    }

    RendererSnapshot {
        wires,
        junctions,
        arcs,
        polygons,
        labels,
        pin_texts: Vec::new(),
        reference_value_texts,
        parameter_texts,
        overlays: OverlayInputs::default(),
        erc_markers: Vec::new(),
        wire_color_overrides: HashMap::new(),
    }
}

#[derive(Clone, Copy)]
pub(super) enum FieldSlot {
    Reference,
    Value,
}

/// Text for a symbol's reference/value field. The stored `TextProp` is the
/// single source (draw and hit-test both read it); the fixed offsets below
/// apply only to symbols that carry no `TextProp`. A hidden field draws
/// nothing.
pub(super) fn field_text_input(
    symbol: &Symbol,
    content: &str,
    prop: Option<&TextProp>,
    slot: FieldSlot,
    color: [f32; 4],
) -> Option<TextInput> {
    if let Some(prop) = prop {
        if prop.hidden {
            return None;
        }
        // Rotation/justification are stored relative to the symbol; resolve
        // them exactly as the SVG exporter does.
        let (rot_deg, h_align, v_align) = field_effective_style(prop, symbol);
        return Some(TextInput {
            content: content.to_owned(),
            position: [prop.position.x as f32, prop.position.y as f32],
            size_mm: prop.font_size as f32,
            color,
            bold: false,
            italic: false,
            rotation_rad: rot_deg.to_radians() as f32,
            h_align,
            v_align,
        });
    }
    let (dy, v_align) = match slot {
        FieldSlot::Reference => (-3.5, VAlign::Bottom),
        FieldSlot::Value => (3.6, VAlign::Top),
    };
    Some(TextInput {
        content: content.to_owned(),
        position: [symbol.position.x as f32, (symbol.position.y + dy) as f32],
        size_mm: 1.05,
        color,
        bold: false,
        italic: false,
        rotation_rad: symbol.rotation.to_radians() as f32,
        h_align: HAlign::Center,
        v_align,
    })
}
