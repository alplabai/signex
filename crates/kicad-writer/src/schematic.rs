use std::collections::BTreeMap;

use signex_types::property::SchematicProperty;
use signex_types::schematic::*;

use crate::sexpr_render::{
    SExpr, at_node, atom, effects_node, hide_yes_node, node, raw, write_rendered_sexpr, yes_no_node,
};

// ---------------------------------------------------------------------------
// Enum-to-KiCad-string helpers
// ---------------------------------------------------------------------------

fn halign_str(a: HAlign) -> &'static str {
    match a {
        HAlign::Left => "left",
        HAlign::Center => "center",
        HAlign::Right => "right",
    }
}

fn valign_str(a: VAlign) -> &'static str {
    match a {
        VAlign::Top => "top",
        VAlign::Center => "center",
        VAlign::Bottom => "bottom",
    }
}

fn fill_type_str(f: FillType) -> &'static str {
    match f {
        FillType::None => "none",
        FillType::Outline => "outline",
        FillType::Background => "background",
    }
}

fn pin_electrical_str(t: PinDirection) -> &'static str {
    match t {
        PinDirection::Input => "input",
        PinDirection::Output => "output",
        PinDirection::Bidirectional => "bidirectional",
        PinDirection::ThreeStatable => "tri_state",
        PinDirection::Passive => "passive",
        PinDirection::PowerInput => "power_in",
        PinDirection::PowerOutput => "power_out",
        PinDirection::OpenDrainLow => "open_collector",
        PinDirection::OpenDrainHigh => "open_emitter",
        PinDirection::DoNotConnect => "no_connect",
        // GroundReference / Differential / Clock / Unclassified — Signex
        // additions or unmapped variants. KiCad's narrower vocabulary lacks
        // direct equivalents, so emit "unspecified" as a safe default.
        PinDirection::GroundReference
        | PinDirection::Differential
        | PinDirection::Clock
        | PinDirection::Unclassified => "unspecified",
    }
}

fn pin_shape_str(s: PinShapeStyle) -> &'static str {
    match s {
        PinShapeStyle::Plain => "line",
        PinShapeStyle::InvertedBubble => "inverted",
        PinShapeStyle::ClockTriangle => "clock",
        PinShapeStyle::InvertedClockBubble => "inverted_clock",
        // Hysteresis* / Schmitt have no KiCad equivalent — emit plain "line"
        // so the file stays well-formed; the visual is lost on export.
        PinShapeStyle::HysteresisInput
        | PinShapeStyle::HysteresisOutput
        | PinShapeStyle::Schmitt => "line",
    }
}

fn label_type_keyword(lt: LabelType) -> &'static str {
    match lt {
        LabelType::Net | LabelType::Power => "label",
        LabelType::Global => "global_label",
        LabelType::Hierarchical => "hierarchical_label",
    }
}

fn stroke_default_node(width: f64) -> SExpr {
    node(
        "stroke",
        vec![
            node("width", vec![atom(width)]),
            node("type", vec![raw("default")]),
        ],
    )
}

fn stroke_colored_node(width: f64, color: Option<StrokeColor>) -> SExpr {
    let mut children = vec![
        node("width", vec![atom(width)]),
        node("type", vec![raw("default")]),
    ];
    if let Some(c) = color {
        children.push(node(
            "color",
            vec![atom(c.r), atom(c.g), atom(c.b), atom(alpha_to_float(c.a))],
        ));
    }
    node("stroke", children)
}

/// KiCad writes alpha as a 0..1 float in `(color ...)` quads; convert
/// our 0..255 byte representation back to that form for round-tripping.
fn alpha_to_float(a: u8) -> f64 {
    a as f64 / 255.0
}

fn fill_type_node(fill: FillType) -> SExpr {
    node("fill", vec![node("type", vec![raw(fill_type_str(fill))])])
}

/// `(fill ...)` for `(sheet ...)` blocks. KiCad serializes the sheet body
/// fill as a literal `(color r g b a)` rather than a `(type ...)` enum, so
/// when a colour is present we emit it; otherwise fall back to the regular
/// fill-type form to keep round-trips clean for sheets without overrides.
fn sheet_fill_node(fill: FillType, color: Option<StrokeColor>) -> SExpr {
    if let Some(c) = color {
        node(
            "fill",
            vec![node(
                "color",
                vec![atom(c.r), atom(c.g), atom(c.b), atom(alpha_to_float(c.a))],
            )],
        )
    } else {
        fill_type_node(fill)
    }
}

fn xy_node(point: Point) -> SExpr {
    node("xy", vec![atom(point.x), atom(point.y)])
}

fn points_node(points: &[Point]) -> SExpr {
    node("pts", points.iter().copied().map(xy_node))
}

fn text_effects_node(
    font_size: f64,
    bold: bool,
    italic: bool,
    justify_h: HAlign,
    justify_v: VAlign,
) -> SExpr {
    let mut extras = Vec::new();
    if justify_h != HAlign::Center || justify_v != VAlign::Center {
        let mut justify_children = Vec::new();
        if justify_h != HAlign::Center {
            justify_children.push(raw(halign_str(justify_h)));
        }
        if justify_v != VAlign::Center {
            justify_children.push(raw(valign_str(justify_v)));
        }
        extras.push(node("justify", justify_children));
    }
    effects_node(font_size, None, bold, italic, extras)
}

fn schematic_property_node(
    key: &str,
    value: &str,
    text: &TextProp,
    show_name: bool,
    do_not_autoplace: bool,
    id: Option<u32>,
) -> SExpr {
    let mut items = vec![
        atom(key),
        atom(value),
        at_node(text.position.x, text.position.y, Some(text.rotation)),
        yes_no_node("show_name", show_name),
        yes_no_node("do_not_autoplace", do_not_autoplace),
    ];
    if let Some(id) = id {
        items.push(node("id", vec![atom(id)]));
    }
    if text.hidden {
        items.push(hide_yes_node());
    }
    items.push(text_effects_node(
        text.font_size,
        false,
        false,
        text.justify_h,
        text.justify_v,
    ));

    node("property", items)
}

fn default_symbol_property_text(position: Point, hidden: bool) -> TextProp {
    TextProp {
        position,
        rotation: 0.0,
        font_size: SCHEMATIC_TEXT_MM,
        justify_h: HAlign::Center,
        justify_v: VAlign::Center,
        hidden,
    }
}

fn custom_property_node(property: &SchematicProperty, fallback_pos: Point) -> SExpr {
    let text = property
        .text
        .clone()
        .unwrap_or_else(|| default_symbol_property_text(fallback_pos, true));

    let mut property_node = schematic_property_node(
        &property.key,
        &property.value,
        &text,
        property.show_name.unwrap_or(false),
        property.do_not_autoplace.unwrap_or(false),
        property.id,
    );

    if !property.variant_overrides.is_empty() {
        let mut variants = Vec::new();
        for (variant_name, variant_value) in &property.variant_overrides {
            variants.push(node("variant", vec![atom(variant_name), atom(variant_value)]));
        }
        if let SExpr::List(items) = &mut property_node {
            items.push(node("variants", variants));
        }
    }

    property_node
}

fn symbol_instances_node(instances: &[SymbolInstance]) -> Option<SExpr> {
    if instances.is_empty() {
        return None;
    }

    let mut grouped: BTreeMap<&str, Vec<&SymbolInstance>> = BTreeMap::new();
    for instance in instances {
        grouped
            .entry(instance.project.as_str())
            .or_default()
            .push(instance);
    }

    let mut projects = Vec::new();
    for (project, project_instances) in grouped {
        let mut project_children = vec![atom(project)];
        let mut sorted_instances = project_instances;
        sorted_instances.sort_by(|left, right| left.path.cmp(&right.path));
        for instance in sorted_instances {
            project_children.push(node(
                "path",
                vec![
                    atom(&instance.path),
                    node("reference", vec![atom(&instance.reference)]),
                    node("unit", vec![atom(instance.unit)]),
                ],
            ));
        }
        projects.push(node("project", project_children));
    }

    Some(node("instances", projects))
}

fn symbol_node(sym: &Symbol) -> SExpr {
    let mut items = vec![
        node("lib_id", vec![atom(&sym.lib_id)]),
        at_node(sym.position.x, sym.position.y, Some(sym.rotation)),
    ];
    if sym.mirror_x {
        items.push(node("mirror", vec![raw("x")]));
    }
    if sym.mirror_y {
        items.push(node("mirror", vec![raw("y")]));
    }

    items.push(node("unit", vec![atom(sym.unit)]));
    if sym.locked {
        items.push(node("locked", Vec::new()));
    }
    items.push(yes_no_node("exclude_from_sim", sym.exclude_from_sim));
    items.push(yes_no_node("in_bom", sym.in_bom));
    items.push(yes_no_node("on_board", sym.on_board));
    items.push(yes_no_node("dnp", sym.dnp));
    if sym.fields_autoplaced {
        items.push(node("fields_autoplaced", Vec::new()));
    }
    items.push(node("uuid", vec![atom(sym.uuid.to_string())]));

    let reference_node = match sym.ref_text.as_ref() {
        Some(ref_text) => {
            schematic_property_node("Reference", &sym.reference, ref_text, false, false, None)
        }
        None => schematic_property_node(
            "Reference",
            &sym.reference,
            &default_symbol_property_text(sym.position, true),
            false,
            false,
            None,
        ),
    };
    items.push(reference_node);

    let value_node = match sym.val_text.as_ref() {
        Some(val_text) => {
            schematic_property_node("Value", &sym.value, val_text, false, false, None)
        }
        None => schematic_property_node(
            "Value",
            &sym.value,
            &default_symbol_property_text(sym.position, false),
            false,
            false,
            None,
        ),
    };
    items.push(value_node);

    items.push(schematic_property_node(
        "Footprint",
        &sym.footprint,
        &default_symbol_property_text(sym.position, true),
        false,
        false,
        None,
    ));
    items.push(schematic_property_node(
        "Datasheet",
        &sym.datasheet,
        &default_symbol_property_text(sym.position, true),
        false,
        false,
        None,
    ));

    let mut custom_properties = sym.custom_properties.clone();
    custom_properties.sort_by(|left, right| left.key.cmp(&right.key));
    for property in &custom_properties {
        items.push(custom_property_node(property, sym.position));
    }

    let custom_keys: std::collections::BTreeSet<&str> = custom_properties
        .iter()
        .map(|property| property.key.as_str())
        .collect();
    let mut field_keys: Vec<_> = sym
        .fields
        .keys()
        .filter(|key| !custom_keys.contains(key.as_str()))
        .collect();
    field_keys.sort();
    for key in field_keys {
        let value = &sym.fields[key];
        items.push(custom_property_node(
            &SchematicProperty {
                key: key.clone(),
                value: value.clone(),
                id: None,
                text: None,
                show_name: Some(false),
                do_not_autoplace: Some(false),
                variant_overrides: Default::default(),
            },
            sym.position,
        ));
    }

    let mut pin_entries: Vec<_> = sym.pin_uuids.iter().collect();
    pin_entries.sort_by(|left, right| left.0.cmp(right.0));
    for (pin_number, pin_uuid) in pin_entries {
        items.push(node(
            "pin",
            vec![
                atom(pin_number),
                node("uuid", vec![atom(pin_uuid.to_string())]),
            ],
        ));
    }

    if let Some(instances_node) = symbol_instances_node(&sym.instances) {
        items.push(instances_node);
    }

    node("symbol", items)
}

fn lib_graphic_node(graphic: &Graphic) -> SExpr {
    match graphic {
        Graphic::Polyline {
            points,
            width,
            fill,
        } => node(
            "polyline",
            vec![
                points_node(points),
                stroke_default_node(*width),
                fill_type_node(*fill),
            ],
        ),
        Graphic::Rectangle {
            start,
            end,
            width,
            fill,
        } => node(
            "rectangle",
            vec![
                node("start", vec![atom(start.x), atom(start.y)]),
                node("end", vec![atom(end.x), atom(end.y)]),
                stroke_default_node(*width),
                fill_type_node(*fill),
            ],
        ),
        Graphic::Circle {
            center,
            radius,
            width,
            fill,
        } => node(
            "circle",
            vec![
                node("center", vec![atom(center.x), atom(center.y)]),
                node("radius", vec![atom(*radius)]),
                stroke_default_node(*width),
                fill_type_node(*fill),
            ],
        ),
        Graphic::Arc {
            start,
            mid,
            end,
            width,
            fill,
        } => node(
            "arc",
            vec![
                node("start", vec![atom(start.x), atom(start.y)]),
                node("mid", vec![atom(mid.x), atom(mid.y)]),
                node("end", vec![atom(end.x), atom(end.y)]),
                stroke_default_node(*width),
                fill_type_node(*fill),
            ],
        ),
        Graphic::Text {
            text,
            position,
            rotation,
            font_size,
            bold,
            italic,
            justify_h,
            justify_v,
        } => node(
            "text",
            vec![
                atom(text),
                at_node(position.x, position.y, Some(*rotation)),
                text_effects_node(*font_size, *bold, *italic, *justify_h, *justify_v),
            ],
        ),
        Graphic::TextBox {
            text,
            position,
            rotation,
            size,
            font_size,
            bold,
            italic,
            width,
            fill,
        } => node(
            "text_box",
            vec![
                atom(text),
                at_node(position.x, position.y, Some(*rotation)),
                node("size", vec![atom(size.x), atom(size.y)]),
                stroke_default_node(*width),
                fill_type_node(*fill),
                effects_node(*font_size, None, *bold, *italic, Vec::new()),
            ],
        ),
        Graphic::Bezier {
            points,
            width,
            fill,
        } => node(
            "bezier",
            vec![
                points_node(points),
                stroke_default_node(*width),
                fill_type_node(*fill),
            ],
        ),
    }
}

fn lib_pin_node(pin: &Pin) -> SExpr {
    let mut items = vec![
        raw(pin_electrical_str(pin.direction)),
        raw(pin_shape_str(pin.shape_style)),
        at_node(pin.position.x, pin.position.y, Some(pin.rotation)),
        node("length", vec![atom(pin.length)]),
    ];
    if !pin.visible {
        items.push(hide_yes_node());
    }

    let name_effects = effects_node(
        SCHEMATIC_TEXT_MM,
        None,
        false,
        false,
        if pin.name_visible {
            Vec::new()
        } else {
            vec![hide_yes_node()]
        },
    );
    let number_effects = effects_node(
        SCHEMATIC_TEXT_MM,
        None,
        false,
        false,
        if pin.number_visible {
            Vec::new()
        } else {
            vec![hide_yes_node()]
        },
    );

    items.push(node("name", vec![atom(&pin.name), name_effects]));
    items.push(node("number", vec![atom(&pin.number), number_effects]));

    node("pin", items)
}

fn lib_symbol_property_node(key: &str, value: &str, id: u32) -> SExpr {
    node(
        "property",
        vec![
            atom(key),
            atom(value),
            node("id", vec![atom(id)]),
            at_node(0.0, 0.0, Some(0.0)),
            effects_node(SCHEMATIC_TEXT_MM, None, false, false, Vec::new()),
        ],
    )
}

fn text_note_node(note: &TextNote) -> SExpr {
    node(
        "text",
        vec![
            atom(note.text.replace('\n', "\\n")),
            yes_no_node("exclude_from_sim", false),
            at_node(note.position.x, note.position.y, Some(note.rotation)),
            effects_node(note.font_size, None, false, false, Vec::new()),
            node("uuid", vec![atom(note.uuid.to_string())]),
        ],
    )
}

fn drawing_node(drawing: &SchDrawing) -> SExpr {
    match drawing {
        SchDrawing::Line {
            uuid,
            start,
            end,
            width,
            stroke_color,
        } => node(
            "polyline",
            vec![
                points_node(&[*start, *end]),
                stroke_colored_node(*width, *stroke_color),
                node("uuid", vec![atom(uuid.to_string())]),
            ],
        ),
        SchDrawing::Polyline {
            uuid,
            points,
            width,
            fill,
            stroke_color,
        } => node(
            "polyline",
            vec![
                points_node(points),
                stroke_colored_node(*width, *stroke_color),
                fill_type_node(*fill),
                node("uuid", vec![atom(uuid.to_string())]),
            ],
        ),
        SchDrawing::Circle {
            uuid,
            center,
            radius,
            width,
            fill,
            stroke_color,
        } => node(
            "circle",
            vec![
                node("center", vec![atom(center.x), atom(center.y)]),
                node("radius", vec![atom(*radius)]),
                stroke_colored_node(*width, *stroke_color),
                fill_type_node(*fill),
                node("uuid", vec![atom(uuid.to_string())]),
            ],
        ),
        SchDrawing::Arc {
            uuid,
            start,
            mid,
            end,
            width,
            fill,
            stroke_color,
        } => node(
            "arc",
            vec![
                node("start", vec![atom(start.x), atom(start.y)]),
                node("mid", vec![atom(mid.x), atom(mid.y)]),
                node("end", vec![atom(end.x), atom(end.y)]),
                stroke_colored_node(*width, *stroke_color),
                fill_type_node(*fill),
                node("uuid", vec![atom(uuid.to_string())]),
            ],
        ),
        SchDrawing::Rect {
            uuid,
            start,
            end,
            width,
            fill,
            stroke_color,
        } => node(
            "rectangle",
            vec![
                node("start", vec![atom(start.x), atom(start.y)]),
                node("end", vec![atom(end.x), atom(end.y)]),
                stroke_colored_node(*width, *stroke_color),
                fill_type_node(*fill),
                node("uuid", vec![atom(uuid.to_string())]),
            ],
        ),
    }
}

fn title_block_node(sheet: &SchematicSheet) -> Option<SExpr> {
    if sheet.title_block.is_empty() {
        return None;
    }

    let mut children = Vec::new();
    if let Some(title) = sheet.title_block.get("title") {
        children.push(node("title", vec![atom(title)]));
    }
    if let Some(date) = sheet.title_block.get("date") {
        children.push(node("date", vec![atom(date)]));
    }
    if let Some(rev) = sheet.title_block.get("rev") {
        children.push(node("rev", vec![atom(rev)]));
    }
    if let Some(company) = sheet.title_block.get("company") {
        children.push(node("company", vec![atom(company)]));
    }
    for i in 1..=9 {
        let key = format!("comment_{}", i);
        if let Some(comment) = sheet.title_block.get(&key) {
            children.push(node("comment", vec![atom(i), atom(comment)]));
        }
    }

    Some(node("title_block", children))
}

fn junction_node(j: &Junction) -> SExpr {
    node(
        "junction",
        vec![
            at_node(j.position.x, j.position.y, None),
            node("diameter", vec![atom(0)]),
            node("color", vec![atom(0), atom(0), atom(0), atom(0)]),
            node("uuid", vec![atom(j.uuid.to_string())]),
        ],
    )
}

fn no_connect_node(nc: &NoConnect) -> SExpr {
    node(
        "no_connect",
        vec![
            at_node(nc.position.x, nc.position.y, None),
            node("uuid", vec![atom(nc.uuid.to_string())]),
        ],
    )
}

fn bus_node(b: &Bus) -> SExpr {
    node(
        "bus",
        vec![
            node("pts", vec![xy_node(b.start), xy_node(b.end)]),
            node(
                "stroke",
                vec![
                    node("width", vec![atom(0)]),
                    node("type", vec![raw("default")]),
                    node("color", vec![atom(0), atom(0), atom(0), atom(0)]),
                ],
            ),
            node("uuid", vec![atom(b.uuid.to_string())]),
        ],
    )
}

fn bus_entry_node(be: &BusEntry) -> SExpr {
    node(
        "bus_entry",
        vec![
            at_node(be.position.x, be.position.y, None),
            node("size", vec![atom(be.size.0), atom(be.size.1)]),
            node(
                "stroke",
                vec![
                    node("width", vec![atom(0)]),
                    node("type", vec![raw("default")]),
                    node("color", vec![atom(0), atom(0), atom(0), atom(0)]),
                ],
            ),
            node("uuid", vec![atom(be.uuid.to_string())]),
        ],
    )
}

fn wire_node(wire: &Wire) -> SExpr {
    node(
        "wire",
        vec![
            node("pts", vec![xy_node(wire.start), xy_node(wire.end)]),
            node(
                "stroke",
                vec![
                    node("width", vec![atom(0)]),
                    node("type", vec![raw("default")]),
                ],
            ),
            node("uuid", vec![atom(wire.uuid.to_string())]),
        ],
    )
}

fn no_erc_node(ne: &NoConnect) -> SExpr {
    node(
        "no_erc",
        vec![
            at_node(ne.position.x, ne.position.y, None),
            node("uuid", vec![atom(ne.uuid.to_string())]),
        ],
    )
}

fn label_node(l: &Label) -> SExpr {
    let keyword = label_type_keyword(l.label_type);
    let mut items = vec![atom(&l.text)];
    if !l.shape.is_empty() {
        items.push(node("shape", vec![raw(l.shape.as_str())]));
    }
    items.push(at_node(l.position.x, l.position.y, Some(l.rotation)));

    // Keep the compact `(justify bottom)` form for the default
    // left-horizontal + bottom-vertical case so round-tripping a KiCad
    // file with no explicit alignment doesn't grow it; otherwise emit
    // the full `(justify <h> <v>)` pair.
    let justify = if l.justify == HAlign::Left && l.justify_v == VAlign::Bottom {
        node("justify", vec![raw("bottom")])
    } else {
        node(
            "justify",
            vec![raw(halign_str(l.justify)), raw(valign_str(l.justify_v))],
        )
    };
    let extras = vec![justify];
    items.push(effects_node(l.font_size, None, false, false, extras));
    items.push(node("uuid", vec![atom(l.uuid.to_string())]));

    node(keyword, items)
}

fn root_sheet_instance_node(sheet: &SchematicSheet) -> SExpr {
    node(
        "path",
        vec![atom("/"), node("page", vec![atom(&sheet.root_sheet_page)])],
    )
}

fn kicad_sch_root_node(sheet: &SchematicSheet) -> SExpr {
    let mut items = vec![
        node("version", vec![atom(sheet.version)]),
        node("generator", vec![atom("signex")]),
        node("generator_version", vec![atom("0.1")]),
        node("uuid", vec![atom(sheet.uuid.to_string())]),
        node("paper", vec![atom(&sheet.paper_size)]),
    ];

    if let Some(title_block) = title_block_node(sheet) {
        items.push(title_block);
    }

    if !sheet.lib_symbols.is_empty() {
        let mut lib_ids: Vec<_> = sheet.lib_symbols.keys().collect();
        lib_ids.sort();
        let mut lib_children = Vec::new();
        for id in lib_ids {
            lib_children.push(lib_symbol_node(&sheet.lib_symbols[id]));
        }
        items.push(node("lib_symbols", lib_children));
    }

    items.extend(sheet.junctions.iter().map(junction_node));
    items.extend(sheet.no_connects.iter().map(no_connect_node));
    items.extend(sheet.buses.iter().map(bus_node));
    items.extend(sheet.bus_entries.iter().map(bus_entry_node));
    items.extend(sheet.wires.iter().map(wire_node));
    items.extend(sheet.labels.iter().map(label_node));
    items.extend(sheet.symbols.iter().map(symbol_node));
    items.extend(sheet.no_erc_directives.iter().map(no_erc_node));
    items.extend(sheet.text_notes.iter().map(text_note_node));
    items.extend(sheet.drawings.iter().map(drawing_node));
    items.extend(sheet.child_sheets.iter().map(child_sheet_node));

    items.push(node(
        "sheet_instances",
        vec![root_sheet_instance_node(sheet)],
    ));

    node("kicad_sch", items)
}

fn effects_with_justify_node(justify_tokens: &[&str]) -> SExpr {
    let mut extras = Vec::new();
    if !justify_tokens.is_empty() {
        extras.push(node("justify", justify_tokens.iter().copied().map(raw)));
    }
    effects_node(SCHEMATIC_TEXT_MM, None, false, false, extras)
}

fn child_sheet_property_node(
    key: &str,
    value: &str,
    id: u32,
    x: f64,
    y: f64,
    justify_tokens: &[&str],
) -> SExpr {
    node(
        "property",
        vec![
            atom(key),
            atom(value),
            node("id", vec![atom(id)]),
            at_node(x, y, Some(0.0)),
            yes_no_node("show_name", false),
            yes_no_node("do_not_autoplace", false),
            effects_with_justify_node(justify_tokens),
        ],
    )
}

fn child_sheet_pin_node(pin: &SheetPin) -> SExpr {
    node(
        "pin",
        vec![
            atom(&pin.name),
            raw(pin.direction.clone()),
            at_node(pin.position.x, pin.position.y, Some(pin.rotation)),
            effects_with_justify_node(&["left"]),
            node("uuid", vec![atom(pin.uuid.to_string())]),
        ],
    )
}

fn sheet_instances_node(instances: &[SheetInstance]) -> Option<SExpr> {
    if instances.is_empty() {
        return None;
    }

    let mut grouped: BTreeMap<&str, Vec<&SheetInstance>> = BTreeMap::new();
    for instance in instances {
        grouped
            .entry(instance.project.as_str())
            .or_default()
            .push(instance);
    }

    let mut projects = Vec::new();
    for (project, project_instances) in grouped {
        let mut project_children = vec![atom(project)];
        let mut sorted_instances = project_instances;
        sorted_instances.sort_by(|left, right| left.path.cmp(&right.path));
        for instance in sorted_instances {
            project_children.push(node(
                "path",
                vec![
                    atom(&instance.path),
                    node("page", vec![atom(&instance.page)]),
                ],
            ));
        }
        projects.push(node("project", project_children));
    }

    Some(node("instances", projects))
}

fn child_sheet_node(cs: &ChildSheet) -> SExpr {
    let mut items = vec![
        at_node(cs.position.x, cs.position.y, None),
        node("size", vec![atom(cs.size.0), atom(cs.size.1)]),
    ];
    if cs.fields_autoplaced {
        items.push(node("fields_autoplaced", Vec::new()));
    }
    items.push(stroke_colored_node(cs.stroke_width, cs.stroke_color));
    items.push(sheet_fill_node(cs.fill, cs.fill_color));
    items.push(node("uuid", vec![atom(cs.uuid.to_string())]));
    items.push(child_sheet_property_node(
        "Sheet name",
        &cs.name,
        0,
        cs.position.x,
        cs.position.y - 1.0,
        &["left", "bottom"],
    ));
    items.push(child_sheet_property_node(
        "Sheet file",
        &cs.filename,
        1,
        cs.position.x,
        cs.position.y + cs.size.1 + 1.0,
        &["left", "top"],
    ));

    for pin in &cs.pins {
        items.push(child_sheet_pin_node(pin));
    }

    if let Some(instances) = sheet_instances_node(&cs.instances) {
        items.push(instances);
    }

    node("sheet", items)
}

fn lib_sub_symbol_node(base_name: &str, unit: u32, body_style: u32, lib: &LibSymbol) -> SExpr {
    let mut children = vec![atom(format!("{}_{}_{}", base_name, unit, body_style))];

    for graphic in lib
        .graphics
        .iter()
        .filter(|graphic| graphic.unit == unit && graphic.body_style == body_style)
    {
        children.push(lib_graphic_node(&graphic.graphic));
    }
    for pin in lib
        .pins
        .iter()
        .filter(|pin| pin.unit == unit && pin.body_style == body_style)
    {
        children.push(lib_pin_node(&pin.pin));
    }

    node("symbol", children)
}

fn lib_symbol_node(lib: &LibSymbol) -> SExpr {
    let mut items = vec![
        atom(&lib.id),
        yes_no_node("in_bom", lib.in_bom),
        yes_no_node("on_board", lib.on_board),
        yes_no_node("in_pos_files", lib.in_pos_files),
        yes_no_node(
            "duplicate_pin_numbers_are_jumpers",
            lib.duplicate_pin_numbers_are_jumpers,
        ),
    ];

    if !lib.show_pin_numbers {
        items.push(node("pin_numbers", vec![raw("hide")]));
    }

    let mut pin_names_children = vec![node("offset", vec![atom(lib.pin_name_offset)])];
    if !lib.show_pin_names {
        pin_names_children.push(raw("hide"));
    }
    items.push(node("pin_names", pin_names_children));

    let base_name = lib.id.split(':').next_back().unwrap_or(&lib.id);
    let reference = if lib.reference.is_empty() {
        "U"
    } else {
        &lib.reference
    };
    let value = if lib.value.is_empty() {
        base_name
    } else {
        &lib.value
    };

    items.push(lib_symbol_property_node("Reference", reference, 0));
    items.push(lib_symbol_property_node("Value", value, 1));
    items.push(lib_symbol_property_node("Footprint", &lib.footprint, 2));
    items.push(lib_symbol_property_node("Datasheet", &lib.datasheet, 3));

    if !lib.description.is_empty() {
        items.push(lib_symbol_property_node("Description", &lib.description, 4));
    }
    if !lib.keywords.is_empty() {
        items.push(lib_symbol_property_node("ki_keywords", &lib.keywords, 5));
    }
    if !lib.fp_filters.is_empty() {
        items.push(lib_symbol_property_node(
            "ki_fp_filters",
            &lib.fp_filters,
            6,
        ));
    }

    let mut sub_keys: std::collections::BTreeSet<(u32, u32)> = std::collections::BTreeSet::new();
    for graphic in &lib.graphics {
        sub_keys.insert((graphic.unit, graphic.body_style));
    }
    for pin in &lib.pins {
        sub_keys.insert((pin.unit, pin.body_style));
    }
    if sub_keys.is_empty() {
        sub_keys.insert((0, 1));
    }

    for (unit, body_style) in sub_keys {
        items.push(lib_sub_symbol_node(base_name, unit, body_style, lib));
    }

    node("symbol", items)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Serialize a [`SchematicSheet`] to the KiCad `.kicad_sch` S-expression format.
pub fn write_schematic(sheet: &SchematicSheet) -> String {
    let mut out = String::with_capacity(64 * 1024);
    write_rendered_sexpr(&mut out, 0, kicad_sch_root_node(sheet));
    out
}

// ---------------------------------------------------------------------------
// Section writers
// ---------------------------------------------------------------------------

#[cfg(test)]
fn write_label(out: &mut String, l: &Label) {
    write_rendered_sexpr(out, 2, label_node(l));
}

// ---------------------------------------------------------------------------
// lib_symbol writer
// ---------------------------------------------------------------------------

#[cfg(test)]
fn write_lib_symbol(out: &mut String, _id: &str, lib: &LibSymbol) {
    write_rendered_sexpr(out, 4, lib_symbol_node(lib));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(expr: SExpr, indent: usize) -> String {
        let mut out = String::new();
        write_rendered_sexpr(&mut out, indent, expr);
        out
    }

    #[test]
    fn writes_not_connected_pins_as_no_connect() {
        assert_eq!(
            pin_electrical_str(PinDirection::DoNotConnect),
            "no_connect"
        );
    }

    #[test]
    fn writes_property_metadata_in_kicad_order() {
        let text = TextProp {
            position: Point { x: 10.0, y: 20.0 },
            rotation: 0.0,
            font_size: SCHEMATIC_TEXT_MM,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
            hidden: false,
        };

        let out = render(
            schematic_property_node("Reference", "R1", &text, false, false, None),
            4,
        );

        assert!(out.contains("(show_name no)"));
        assert!(out.contains("(do_not_autoplace no)"));
        assert!(out.contains("(effects (font (size 1.27 1.27)))"));
    }

    #[test]
    fn writes_custom_property_with_ast_metadata() {
        let property = SchematicProperty {
            key: "Tolerance".to_string(),
            value: "1%".to_string(),
            id: Some(7),
            text: Some(TextProp {
                position: Point { x: 110.0, y: 60.0 },
                rotation: 90.0,
                font_size: 1.5,
                justify_h: HAlign::Left,
                justify_v: VAlign::Bottom,
                hidden: true,
            }),
            show_name: Some(true),
            do_not_autoplace: Some(true),
            variant_overrides: Default::default(),
        };

        let out = render(custom_property_node(&property, Point { x: 0.0, y: 0.0 }), 4);

        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();
        assert_eq!(parsed.keyword(), Some("property"));
        assert_eq!(parsed.first_arg(), Some("Tolerance"));
        assert_eq!(parsed.arg(1), Some("1%"));
        assert_eq!(
            parsed.find("id").and_then(|node| node.first_arg()),
            Some("7")
        );
        assert_eq!(
            parsed.find("show_name").and_then(|node| node.first_arg()),
            Some("yes")
        );
        assert_eq!(
            parsed
                .find("do_not_autoplace")
                .and_then(|node| node.first_arg()),
            Some("yes")
        );
        assert!(parsed.find("hide").is_some());
    }

    #[test]
    fn writes_custom_property_variant_overrides() {
        let mut property = SchematicProperty {
            key: "Fitted".to_string(),
            value: "yes".to_string(),
            id: None,
            text: Some(TextProp {
                position: Point { x: 100.0, y: 50.0 },
                rotation: 0.0,
                font_size: 1.27,
                justify_h: HAlign::Center,
                justify_v: VAlign::Center,
                hidden: true,
            }),
            show_name: Some(false),
            do_not_autoplace: Some(false),
            variant_overrides: Default::default(),
        };
        property
            .variant_overrides
            .insert("DEFAULT".to_string(), "yes".to_string());
        property
            .variant_overrides
            .insert("LITE".to_string(), "no".to_string());

        let out = render(custom_property_node(&property, Point { x: 0.0, y: 0.0 }), 4);
        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();

        let variants = parsed.find("variants").unwrap();
        let variant_nodes = variants.find_all("variant");
        assert_eq!(variant_nodes.len(), 2);

        let default = variant_nodes
            .iter()
            .find(|node| node.first_arg() == Some("DEFAULT"))
            .unwrap();
        let lite = variant_nodes
            .iter()
            .find(|node| node.first_arg() == Some("LITE"))
            .unwrap();

        assert_eq!(default.arg(1), Some("yes"));
        assert_eq!(lite.arg(1), Some("no"));
    }

    #[test]
    fn writes_sheet_instances_root_page_and_symbol_instances() {
        let mut sheet = SchematicSheet {
            uuid: Default::default(),
            version: 20231120,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "7".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: BTreeMap::new().into_iter().collect(),
            lib_symbols: BTreeMap::new().into_iter().collect(),
        };

        sheet.symbols.push(Symbol {
            uuid: Default::default(),
            lib_id: "Device:R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            footprint: String::new(),
            datasheet: "https://example.invalid/r1".to_string(),
            position: Point { x: 10.0, y: 10.0 },
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            unit: 1,
            is_power: false,
            ref_text: Some(TextProp {
                position: Point { x: 10.0, y: 8.0 },
                rotation: 0.0,
                font_size: SCHEMATIC_TEXT_MM,
                justify_h: HAlign::Center,
                justify_v: VAlign::Center,
                hidden: false,
            }),
            val_text: Some(TextProp {
                position: Point { x: 10.0, y: 12.0 },
                rotation: 0.0,
                font_size: SCHEMATIC_TEXT_MM,
                justify_h: HAlign::Center,
                justify_v: VAlign::Center,
                hidden: false,
            }),
            fields_autoplaced: true,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: Vec::new(),
            pin_uuids: [("1".to_string(), Default::default())]
                .into_iter()
                .collect(),
            instances: vec![SymbolInstance {
                project: "GateMagic".to_string(),
                path: "/root".to_string(),
                reference: "R1".to_string(),
                unit: 1,
            }],
        });

        let rendered = write_schematic(&sheet);
        let parsed = kicad_parser::sexpr::parse(&rendered).unwrap();
        assert!(rendered.contains("(sheet_instances"));
        assert!(rendered.contains("(page \"7\")"));
        let symbol = parsed.find("symbol").unwrap();
        let datasheet = symbol
            .find_all("property")
            .into_iter()
            .find(|property| property.first_arg() == Some("Datasheet"))
            .unwrap();
        assert_eq!(datasheet.arg(1), Some("https://example.invalid/r1"));
        assert!(rendered.contains("(pin \"1\" (uuid \"00000000-0000-0000-0000-000000000000\"))"));
        assert!(rendered.contains("(instances"));
        assert!(rendered.contains("(project \"GateMagic\""));
    }

    #[test]
    fn writes_lib_symbol_parent_metadata() {
        let mut out = String::new();
        let lib = LibSymbol {
            id: "Interface_Ethernet:W5500".to_string(),
            reference: "U".to_string(),
            value: "W5500".to_string(),
            footprint: "Package_QFP:LQFP-48_7x7mm_P0.5mm".to_string(),
            datasheet: "http://example.invalid/ds.pdf".to_string(),
            description: "Ethernet controller".to_string(),
            keywords: "WIZnet Ethernet".to_string(),
            fp_filters: "LQFP*".to_string(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: Vec::new(),
            pins: Vec::new(),
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.508,
        };

        write_lib_symbol(&mut out, &lib.id, &lib);

        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();
        assert_eq!(parsed.keyword(), Some("symbol"));
        assert!(
            parsed
                .find_all("property")
                .iter()
                .any(|p| p.first_arg() == Some("Description"))
        );
        assert!(
            parsed
                .find_all("property")
                .iter()
                .any(|p| p.first_arg() == Some("ki_keywords"))
        );
        assert!(
            parsed
                .find_all("property")
                .iter()
                .any(|p| p.first_arg() == Some("ki_fp_filters"))
        );
    }

    #[test]
    fn writes_hidden_lib_pin_flag() {
        let pin = Pin {
            direction: PinDirection::DoNotConnect,
            shape_style: PinShapeStyle::Plain,
            position: Point { x: 20.32, y: 0.0 },
            rotation: 0.0,
            length: 0.0,
            name: "NC".to_string(),
            number: "7".to_string(),
            visible: false,
            name_visible: true,
            number_visible: true,
        };

        let out = render(lib_pin_node(&pin), 8);
        assert!(out.contains("(hide yes)"));
    }

    #[test]
    fn writes_global_label_without_forced_left_justify() {
        let mut out = String::new();
        let label = Label {
            uuid: Default::default(),
            text: "NET_A".to_string(),
            position: Point { x: 10.0, y: 10.0 },
            rotation: 0.0,
            label_type: LabelType::Global,
            shape: "input".to_string(),
            font_size: SCHEMATIC_TEXT_MM,
            justify: HAlign::Left,
            justify_v: VAlign::Bottom,
        };

        write_label(&mut out, &label);

        assert!(!out.contains("(justify left)"));
        assert!(out.contains("(justify bottom)"));
    }

    #[test]
    fn writes_right_aligned_label_with_bottom_vertical_justify() {
        let mut out = String::new();
        let label = Label {
            uuid: Default::default(),
            text: "NET_B".to_string(),
            position: Point { x: 20.0, y: 5.0 },
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: SCHEMATIC_TEXT_MM,
            justify: HAlign::Right,
            justify_v: VAlign::Bottom,
        };

        write_label(&mut out, &label);

        assert!(out.contains("(justify right bottom)"));
    }

    #[test]
    fn roundtrip_preserves_label_vertical_justify() {
        let content = r#"(kicad_sch
    (version 20231120)
    (generator "eeschema")
    (uuid "00000000-0000-0000-0000-000000000001")
    (paper "A4")
    (label "DIVIDED-S_{1}"
        (at 20 20 90)
        (effects (font (size 1.27 1.27)) (justify right top))
        (uuid "00000000-0000-0000-0000-000000000002")
    )
)"#;

        let parsed = kicad_parser::parse_schematic(content).expect("parse");
        let rendered = write_schematic(&parsed);
        let reparsed = kicad_parser::parse_schematic(&rendered).expect("reparse");

        assert_eq!(reparsed.labels.len(), 1);
        assert_eq!(reparsed.labels[0].justify, HAlign::Right);
        assert_eq!(reparsed.labels[0].justify_v, VAlign::Top);
    }

    #[test]
    fn writes_text_note_as_expected_sexpr() {
        let note = TextNote {
            uuid: Default::default(),
            text: "Line1\nLine2".to_string(),
            position: Point { x: 10.0, y: 12.0 },
            rotation: 45.0,
            font_size: 1.4,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
        };

        let out = render(text_note_node(&note), 2);
        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();
        assert_eq!(parsed.keyword(), Some("text"));
        assert_eq!(parsed.first_arg(), Some("Line1\\nLine2"));
        assert!(parsed.find("exclude_from_sim").is_some());
    }

    #[test]
    fn writes_drawing_rect_as_expected_sexpr() {
        let drawing = SchDrawing::Rect {
            uuid: Default::default(),
            start: Point { x: 1.0, y: 2.0 },
            end: Point { x: 3.0, y: 4.0 },
            width: 0.15,
            fill: FillType::Background,
            stroke_color: None,
        };

        let out = render(drawing_node(&drawing), 2);
        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();
        assert_eq!(parsed.keyword(), Some("rectangle"));
        assert!(parsed.find("fill").is_some());
    }

    #[test]
    fn writes_child_sheet_with_instances_as_expected_sexpr() {
        let child = ChildSheet {
            uuid: Default::default(),
            name: "Power".to_string(),
            filename: "power.kicad_sch".to_string(),
            position: Point { x: 10.0, y: 20.0 },
            size: (30.0, 40.0),
            stroke_width: 0.12,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: true,
            pins: vec![SheetPin {
                uuid: Default::default(),
                name: "VIN".to_string(),
                direction: "input".to_string(),
                position: Point { x: 12.0, y: 22.0 },
                rotation: 0.0,
                auto_generated: true,
                user_moved: false,
            }],
            instances: vec![SheetInstance {
                project: "Main".to_string(),
                path: "/sheet-1".to_string(),
                page: "2".to_string(),
            }],
        };

        let out = render(child_sheet_node(&child), 2);
        let parsed = kicad_parser::sexpr::parse(out.trim()).unwrap();
        assert_eq!(parsed.keyword(), Some("sheet"));
        assert!(parsed.find("instances").is_some());
        assert!(
            parsed
                .find_all("property")
                .iter()
                .any(|p| p.first_arg() == Some("Sheet name"))
        );
    }
}
