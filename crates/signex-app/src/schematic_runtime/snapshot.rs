use super::*;

pub fn build_renderer_snapshot(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    overrides: SceneOverrides<'_>,
) -> RendererSnapshot {
    let mut wires = Vec::new();
    let mut wire_line_styles = HashMap::new();
    let mut junctions = Vec::with_capacity(snapshot.junctions.len());
    let mut arcs = Vec::new();
    let mut polygons = Vec::new();
    let mut labels = Vec::new();
    let mut reference_value_texts = Vec::new();
    let mut parameter_texts = Vec::new();
    let document_family = snapshot.font_family.as_deref();

    for wire in &snapshot.wires {
        let p0 = transform.world_to_screen((wire.start.x, wire.start.y));
        let p1 = transform.world_to_screen((wire.end.x, wire.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }

        let class = overrides
            .wire_styling
            .and_then(|styling| styling.class_styles.get(&wire.uuid));
        let base_color = wire_color(overrides.wire_styling, wire.uuid)
            .as_ref()
            .map(to_iced)
            .unwrap_or_else(|| to_iced(&colors.wire));
        let color = focus_color(base_color, focus_set, wire.uuid);
        wires.push(WireInput {
            id: renderer_id(wire.uuid),
            p0: [wire.start.x as f32, wire.start.y as f32],
            p1: [wire.end.x as f32, wire.end.y as f32],
            width_mm: drawn_wire_width_mm(wire, class) as f32,
            explicit_color: Some(to_rgba(color)),
        });
        if let Some(class) = class.filter(|c| c.line != NetLineStyle::Solid) {
            wire_line_styles.insert(renderer_id(wire.uuid), class.line);
        }
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

    // A bus entry is the wire-side stub that meets a bus: wire colour, wire
    // weight, from its bus end (`position`) to `position + size`.
    for entry in &snapshot.bus_entries {
        let end = entry.end();
        let p0 = transform.world_to_screen((entry.position.x, entry.position.y));
        let p1 = transform.world_to_screen((end.x, end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }

        wires.push(WireInput {
            id: renderer_id(entry.uuid),
            p0: [entry.position.x as f32, entry.position.y as f32],
            p1: [end.x as f32, end.y as f32],
            width_mm: signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_MM as f32,
            explicit_color: Some(to_rgba(focus_color(
                to_iced(&colors.wire),
                focus_set,
                entry.uuid,
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
                junction_color(overrides.wire_styling, junction.uuid)
                    .as_ref()
                    .map(to_iced)
                    .unwrap_or_else(|| to_iced(&colors.junction)),
                focus_set,
                junction.uuid,
            )),
        });
    }

    for symbol in &snapshot.symbols {
        let (primitives, bbox) = symbol::symbol_geometry(snapshot, symbol);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }

        let stroke_color = focus_color(to_iced(&colors.body), focus_set, symbol.uuid);
        let style = symbol::SymbolStyle {
            id: renderer_id(symbol.uuid),
            stroke: stroke_color,
            fill: focus_color(to_iced(&colors.body_fill), focus_set, symbol.uuid),
            pin: focus_color(to_iced(&colors.pin), focus_set, symbol.uuid),
            missing: focus_color(to_iced(&colors.no_connect), focus_set, symbol.uuid),
            scale: transform.scale,
            family: resolve_family(None, document_family),
        };
        let mut sink = symbol::SymbolSink {
            wires: &mut wires,
            arcs: &mut arcs,
            polygons: &mut polygons,
            texts: &mut parameter_texts,
        };
        if primitives.is_empty() {
            symbol::push_fallback_box(&bbox, &symbol.lib_id, &style, &mut sink);
        } else {
            symbol::push_primitives(&primitives, &style, &mut sink);
        }

        let value_color = focus_color(to_iced(&colors.value), focus_set, symbol.uuid);
        let reference = overrides
            .designators
            .and_then(|d| d.get(&symbol.uuid))
            .unwrap_or(&symbol.reference);
        if !reference.is_empty()
            && let Some(text) = field_text_input(
                symbol,
                reference,
                symbol.ref_text.as_ref(),
                FieldSlot::Reference,
                to_rgba(stroke_color),
                document_family,
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
                document_family,
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
            family: resolve_family(None, document_family),
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

    let mut drawings = Vec::with_capacity(snapshot.drawings.len());
    let mut drawings_front = Vec::new();
    for drawing in &snapshot.drawings {
        let bbox = drawing_aabb(drawing);
        let min = transform.world_to_screen((bbox.min_x, bbox.min_y));
        let max = transform.world_to_screen((bbox.max_x, bbox.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }
        let out = match snapshot.drawing_placement(drawing.uuid()) {
            DrawingPlacement::Behind => &mut drawings,
            DrawingPlacement::Above => &mut drawings_front,
        };
        drawings::push_drawing(drawing, colors, focus_set, transform.scale, out);
    }

    for label in &snapshot.labels {
        let screen = transform.world_to_screen((label.position.x, label.position.y));
        if !text_anchor_visible(
            screen,
            bounds,
            22.0,
            &label_aabb(label, document_family),
            transform.scale,
        ) {
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
                document_family,
            ));
            let ((tx, ty), rotation, h_align) = label::flag_text(label, document_family);
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
                family: resolve_family(label.font_family.as_deref(), document_family),
                rotation_rad: rotation.to_radians() as f32,
                h_align,
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
                family: resolve_family(label.font_family.as_deref(), document_family),
                rotation_rad: label.rotation.to_radians() as f32,
                h_align: label.justify,
                v_align: label.justify_v,
            });
        }
    }

    for note in &snapshot.text_notes {
        let pos = transform.world_to_screen((note.position.x, note.position.y));
        if !text_anchor_visible(
            pos,
            bounds,
            28.0,
            &note_aabb(note, document_family),
            transform.scale,
        ) {
            continue;
        }

        parameter_texts.push(TextInput {
            content: match overrides.special_strings {
                Some(special) => special.resolve(&note.text),
                None => note.text.clone(),
            },
            position: [note.position.x as f32, note.position.y as f32],
            size_mm: note
                .font_size
                .max(signex_types::schematic::SCHEMATIC_TEXT_MM) as f32,
            color: to_rgba(focus_color(to_iced(&colors.value), focus_set, note.uuid)),
            bold: false,
            italic: false,
            family: resolve_family(note.font_family.as_deref(), document_family),
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
        drawings,
        drawings_front,
        wire_color_overrides: HashMap::new(),
        wire_line_styles,
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
    document_family: Option<&str>,
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
            family: resolve_family(prop.font_family.as_deref(), document_family),
            rotation_rad: rot_deg.to_radians() as f32,
            h_align,
            v_align,
        });
    }
    // A power port is a net name, not a part: a placed one carries no
    // reference prop, so its `#PWR?` designator must not fall back to a
    // default position (#1771).
    if symbol.is_power && matches!(slot, FieldSlot::Reference) {
        return None;
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
        family: resolve_family(None, document_family),
        rotation_rad: symbol.rotation.to_radians() as f32,
        h_align: HAlign::Center,
        v_align,
    })
}

/// Stroke of an arc pushed by [`push_arc`].
pub(super) struct ArcStroke {
    pub id: u64,
    pub width_mm: f32,
    pub color: [f32; 4],
}

/// Where the arc from `start` through `mid` to `end` lies; `None` when the
/// points are collinear and it degenerates to two straight segments.
pub(super) struct ArcGeometry {
    pub center: [f32; 2],
    pub radius_mm: f32,
    pub start_angle_rad: f32,
    pub end_angle_rad: f32,
}

pub(super) fn arc_geometry([start, mid, end]: [Point; 3], scale: f32) -> Option<ArcGeometry> {
    let (cx, cy, r) = circumcircle(start, mid, end)?;
    let a0 = (start.y - cy).atan2(start.x - cx);
    let am = (mid.y - cy).atan2(mid.x - cx);
    let a1 = (end.y - cy).atan2(end.x - cx);
    let (start_angle, end_angle) = if arc_sweeps_through_mid(a0, am, a1) {
        (a0, a1)
    } else {
        (a1, a0)
    };
    Some(ArcGeometry {
        center: [cx as f32, cy as f32],
        radius_mm: r.max(screen_px_to_world_mm(
            signex_types::schematic::SCHEMATIC_RENDER_DRAWING_MIN_ARC_RADIUS_PX,
            scale,
        )) as f32,
        start_angle_rad: start_angle as f32,
        end_angle_rad: end_angle as f32,
    })
}

/// Push the arc from `start` through `mid` to `end`; collinear points fall
/// back to the two straight segments.
pub(super) fn push_arc(
    points @ [start, mid, end]: [Point; 3],
    stroke: ArcStroke,
    scale: f32,
    arcs: &mut Vec<ArcInput>,
    wires: &mut Vec<WireInput>,
) {
    let Some(geometry) = arc_geometry(points, scale) else {
        for (i, (p0, p1)) in [(start, mid), (mid, end)].into_iter().enumerate() {
            wires.push(WireInput {
                id: stroke.id.saturating_add(i as u64),
                p0: [p0.x as f32, p0.y as f32],
                p1: [p1.x as f32, p1.y as f32],
                width_mm: stroke.width_mm,
                explicit_color: Some(stroke.color),
            });
        }
        return;
    };
    arcs.push(ArcInput {
        center: geometry.center,
        radius_mm: geometry.radius_mm,
        start_angle_rad: geometry.start_angle_rad,
        end_angle_rad: geometry.end_angle_rad,
        width_mm: stroke.width_mm,
        color: stroke.color,
    });
}
