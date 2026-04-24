use signex_types::pcb::{
    BoardGraphic, BoardText, Footprint, FpGraphic, LayerDef, NetDef, PCB_DEFAULT_TEXT_SIZE_MM,
    PCB_FP_TEXT_OFFSET_MM, PCB_TEXT_THICKNESS_MM, Pad, PadShape, PadType, PcbBoard, PcbSetup,
    Point, Segment, Via, ViaType, Zone,
};
use signex_types::property::PcbProperty;

use crate::sexpr_render::{
    SExpr, atom, effects_node, hide_yes_node, node, raw, write_rendered_sexpr,
};

// ---------------------------------------------------------------------------
// Enum-to-KiCad-string helpers
// ---------------------------------------------------------------------------

fn pad_type_str(t: PadType) -> &'static str {
    match t {
        PadType::Thru => "thru_hole",
        PadType::Smd => "smd",
        PadType::Connect => "connect",
        PadType::NpThru => "np_thru_hole",
    }
}

fn pad_shape_str(s: PadShape) -> &'static str {
    match s {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::Oval => "oval",
        PadShape::Trapezoid => "trapezoid",
        PadShape::RoundRect => "roundrect",
        PadShape::Custom => "custom",
    }
}

fn via_type_str(v: ViaType) -> &'static str {
    match v {
        ViaType::Through => "via",
        ViaType::Blind => "via_blind",
        ViaType::Micro => "via_micro",
    }
}

// ---------------------------------------------------------------------------
// Float formatting: strip trailing zeros for cleaner output
// ---------------------------------------------------------------------------

fn fmt_f64(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        let s = format!("{:.6}", v);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Low-level node builders
// ---------------------------------------------------------------------------

/// Numeric value through `fmt_f64` (strips trailing zeros, no f64 Display quirks).
fn coord(v: f64) -> SExpr {
    raw(fmt_f64(v))
}

/// `(at X Y [ROT])` using coord() for consistent float formatting.
fn at_coord(x: f64, y: f64, rotation: Option<f64>) -> SExpr {
    let mut items = vec![coord(x), coord(y)];
    if let Some(r) = rotation {
        items.push(coord(r));
    }
    node("at", items)
}

/// `(uuid "UUID")`.
fn uuid_node(u: impl std::fmt::Display) -> SExpr {
    node("uuid", [atom(u.to_string())])
}

/// `(layer "NAME")`.
fn layer_node(l: &str) -> SExpr {
    node("layer", [atom(l)])
}

/// `(stroke (width W) (type default))`.
fn stroke_node(width: f64) -> SExpr {
    node(
        "stroke",
        [
            node("width", [coord(width)]),
            node("type", [raw("default")]),
        ],
    )
}

/// `(pts (xy X Y) ...)`.
fn pts_node(points: &[Point]) -> SExpr {
    node(
        "pts",
        points.iter().map(|p| node("xy", [coord(p.x), coord(p.y)])),
    )
}

// ---------------------------------------------------------------------------
// PCB text helpers
// ---------------------------------------------------------------------------

fn pcb_text_effects_node(font_size: f64) -> SExpr {
    effects_node(
        font_size,
        Some(PCB_TEXT_THICKNESS_MM),
        false,
        false,
        Vec::new(),
    )
}

fn pcb_property_node(property: &PcbProperty) -> SExpr {
    let position = property.position.unwrap_or(Point { x: 0.0, y: 0.0 });
    let mut items: Vec<SExpr> = vec![atom(&property.key), atom(&property.value)];
    items.push(at_coord(
        position.x,
        position.y,
        (property.rotation != 0.0).then_some(property.rotation),
    ));
    if let Some(layer) = &property.layer {
        items.push(layer_node(layer));
    }
    if property.hidden {
        items.push(hide_yes_node());
    }
    items.push(pcb_text_effects_node(
        property.font_size.unwrap_or(PCB_DEFAULT_TEXT_SIZE_MM),
    ));
    node("property", items)
}

fn board_text_node(text: &BoardText) -> SExpr {
    let mut items: Vec<SExpr> = vec![atom(&text.text)];
    items.push(at_coord(
        text.position.x,
        text.position.y,
        (text.rotation != 0.0).then_some(text.rotation),
    ));
    items.push(layer_node(&text.layer));
    items.push(pcb_text_effects_node(text.font_size));
    items.push(uuid_node(text.uuid));
    node("gr_text", items)
}

// ---------------------------------------------------------------------------
// Section builders
// ---------------------------------------------------------------------------

fn build_general(board: &PcbBoard) -> SExpr {
    node(
        "general",
        [
            node("thickness", [coord(board.thickness)]),
            uuid_node(&board.uuid),
        ],
    )
}

fn build_layer(l: &LayerDef) -> SExpr {
    // (ID "NAME" TYPE) — ID and TYPE are unquoted raw tokens, NAME is quoted.
    SExpr::List(vec![
        raw(l.id.to_string()),
        atom(&l.name),
        raw(&l.layer_type),
    ])
}

fn build_layers(layers: &[LayerDef]) -> SExpr {
    node("layers", layers.iter().map(build_layer))
}

fn build_setup(_setup: &PcbSetup) -> SExpr {
    node(
        "setup",
        [
            node("pad_to_mask_clearance", [raw("0")]),
            node(
                "pcbplotparams",
                [
                    node("layerselection", [raw("0x00010fc_ffffffff")]),
                    node("plot_on_all_layers_selection", [raw("0x0000000_00000000")]),
                ],
            ),
        ],
    )
}

fn build_net_class(setup: &PcbSetup) -> SExpr {
    node(
        "net_class",
        [
            atom("Default"),
            atom(""),
            node("clearance", [coord(setup.clearance)]),
            node("trace_width", [coord(setup.trace_width)]),
            node("via_dia", [coord(setup.via_diameter)]),
            node("via_drill", [coord(setup.via_drill)]),
            node("uvia_dia", [coord(setup.via_min_diameter)]),
            node("uvia_drill", [coord(setup.via_min_drill)]),
        ],
    )
}

fn build_net(net: &NetDef) -> SExpr {
    SExpr::List(vec![
        raw("net"),
        raw(net.number.to_string()),
        atom(&net.name),
    ])
}

fn build_segment(s: &Segment) -> SExpr {
    node(
        "segment",
        [
            node("start", [coord(s.start.x), coord(s.start.y)]),
            node("end", [coord(s.end.x), coord(s.end.y)]),
            node("width", [coord(s.width)]),
            layer_node(&s.layer),
            node("net", [raw(s.net.to_string())]),
            uuid_node(s.uuid),
        ],
    )
}

fn build_via(v: &Via) -> SExpr {
    let mut items: Vec<SExpr> = vec![
        at_coord(v.position.x, v.position.y, None),
        node("size", [coord(v.diameter)]),
        node("drill", [coord(v.drill)]),
    ];
    if v.layers.len() >= 2 {
        items.push(node("layers", [atom(&v.layers[0]), atom(&v.layers[1])]));
    }
    items.push(node("net", [raw(v.net.to_string())]));
    items.push(uuid_node(v.uuid));
    node(via_type_str(v.via_type), items)
}

fn build_zone(z: &Zone) -> SExpr {
    let mut items: Vec<SExpr> = vec![
        node("net", [raw(z.net.to_string())]),
        node("net_name", [atom(&z.net_name)]),
        layer_node(&z.layer),
        uuid_node(z.uuid),
    ];
    if z.priority > 0 {
        items.push(node("priority", [raw(z.priority.to_string())]));
    }
    let mut fill_items: Vec<SExpr> = Vec::new();
    if z.thermal_relief {
        fill_items.push(node("thermal_relief", []));
        fill_items.push(node("thermal_gap", [coord(z.thermal_gap)]));
        fill_items.push(node("thermal_bridge_width", [coord(z.thermal_width)]));
    }
    items.push(node("fill", fill_items));
    items.push(node("min_thickness", [coord(z.min_thickness)]));
    if z.clearance > 0.0 {
        items.push(node("clearance", [coord(z.clearance)]));
    }
    if !z.outline.is_empty() {
        items.push(node("polygon", [pts_node(&z.outline)]));
    }
    node("zone", items)
}

fn build_board_graphic(g: &BoardGraphic) -> Option<SExpr> {
    match g.graphic_type.as_str() {
        "line" => {
            let (s, e) = (g.start.as_ref()?, g.end.as_ref()?);
            Some(node(
                "gr_line",
                [
                    node("start", [coord(s.x), coord(s.y)]),
                    node("end", [coord(e.x), coord(e.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        "rect" => {
            let (s, e) = (g.start.as_ref()?, g.end.as_ref()?);
            Some(node(
                "gr_rect",
                [
                    node("start", [coord(s.x), coord(s.y)]),
                    node("end", [coord(e.x), coord(e.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        "circle" => {
            let c = g.center.as_ref()?;
            Some(node(
                "gr_circle",
                [
                    node("center", [coord(c.x), coord(c.y)]),
                    node("end", [coord(c.x + g.radius), coord(c.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        "arc" => {
            let (s, e) = (g.start.as_ref()?, g.end.as_ref()?);
            Some(node(
                "gr_arc",
                [
                    node("start", [coord(s.x), coord(s.y)]),
                    node("end", [coord(e.x), coord(e.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        _ => None,
    }
}

fn build_fp_graphic(g: &FpGraphic) -> Option<SExpr> {
    match g.graphic_type.as_str() {
        "line" => {
            let (s, e) = (g.start.as_ref()?, g.end.as_ref()?);
            Some(node(
                "fp_line",
                [
                    node("start", [coord(s.x), coord(s.y)]),
                    node("end", [coord(e.x), coord(e.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        "rect" => {
            let (s, e) = (g.start.as_ref()?, g.end.as_ref()?);
            let mut items = vec![
                node("start", [coord(s.x), coord(s.y)]),
                node("end", [coord(e.x), coord(e.y)]),
                stroke_node(g.width),
            ];
            if g.fill == "solid" {
                items.push(node("fill", [raw("solid")]));
            }
            items.push(layer_node(&g.layer));
            Some(node("fp_rect", items))
        }
        "circle" => {
            let c = g.center.as_ref()?;
            let mut items = vec![
                node("center", [coord(c.x), coord(c.y)]),
                node("end", [coord(c.x + g.radius), coord(c.y)]),
                stroke_node(g.width),
            ];
            if g.fill == "solid" {
                items.push(node("fill", [raw("solid")]));
            }
            items.push(layer_node(&g.layer));
            Some(node("fp_circle", items))
        }
        "arc" => {
            let (s, m, e) = (g.start.as_ref()?, g.mid.as_ref()?, g.end.as_ref()?);
            Some(node(
                "fp_arc",
                [
                    node("start", [coord(s.x), coord(s.y)]),
                    node("mid", [coord(m.x), coord(m.y)]),
                    node("end", [coord(e.x), coord(e.y)]),
                    stroke_node(g.width),
                    layer_node(&g.layer),
                ],
            ))
        }
        "poly" => {
            if g.points.len() < 2 {
                return None;
            }
            let mut items = vec![pts_node(&g.points), stroke_node(g.width)];
            if g.fill == "solid" {
                items.push(node("fill", [raw("solid")]));
            }
            items.push(layer_node(&g.layer));
            Some(node("fp_poly", items))
        }
        "text" => {
            let pos = g.position.as_ref()?;
            let fs = if g.font_size != 0.0 {
                g.font_size
            } else {
                PCB_DEFAULT_TEXT_SIZE_MM
            };
            Some(node(
                "fp_text",
                [
                    raw("user"),
                    atom(&g.text),
                    at_coord(pos.x, pos.y, (g.rotation != 0.0).then_some(g.rotation)),
                    layer_node(&g.layer),
                    pcb_text_effects_node(fs),
                ],
            ))
        }
        _ => None,
    }
}

fn build_fp_pad(p: &Pad) -> SExpr {
    let mut items: Vec<SExpr> = vec![
        atom(&p.number),
        raw(pad_type_str(p.pad_type)),
        raw(pad_shape_str(p.shape)),
        at_coord(p.position.x, p.position.y, None),
        node("size", [coord(p.size.x), coord(p.size.y)]),
    ];
    if let Some(ref drill) = p.drill {
        if !drill.shape.is_empty() {
            items.push(node("drill", [raw(&drill.shape), coord(drill.diameter)]));
        } else {
            items.push(node("drill", [coord(drill.diameter)]));
        }
    }
    items.push(node("layers", p.layers.iter().map(|l| atom(l))));
    if p.roundrect_ratio != 0.0 {
        items.push(node("roundrect_rratio", [coord(p.roundrect_ratio)]));
    }
    if let Some(ref net) = p.net {
        items.push(node("net", [raw(net.number.to_string()), atom(&net.name)]));
    }
    items.push(uuid_node(p.uuid));
    node("pad", items)
}

fn build_footprint(fp: &Footprint) -> SExpr {
    let mut items: Vec<SExpr> = vec![atom(&fp.footprint_id)];
    if fp.locked {
        items.push(node("locked", [raw("yes")]));
    }
    items.push(layer_node(&fp.layer));
    items.push(at_coord(
        fp.position.x,
        fp.position.y,
        (fp.rotation != 0.0).then_some(fp.rotation),
    ));
    items.push(uuid_node(fp.uuid));

    let properties = effective_footprint_properties(fp);
    for property in &properties {
        items.push(pcb_property_node(property));
    }
    for g in &fp.graphics {
        if is_property_backed_text_graphic(g, &properties) {
            continue;
        }
        if let Some(expr) = build_fp_graphic(g) {
            items.push(expr);
        }
    }
    for p in &fp.pads {
        items.push(build_fp_pad(p));
    }

    node("footprint", items)
}

// ---------------------------------------------------------------------------
// Footprint property helpers (unchanged logic, uses pcb_property_node above)
// ---------------------------------------------------------------------------

fn effective_footprint_properties(fp: &Footprint) -> Vec<PcbProperty> {
    if fp.properties.is_empty() {
        return vec![
            PcbProperty {
                key: "Reference".to_string(),
                value: fp.reference.clone(),
                position: Some(Point {
                    x: 0.0,
                    y: -PCB_FP_TEXT_OFFSET_MM,
                }),
                rotation: 0.0,
                layer: Some("F.SilkS".to_string()),
                font_size: Some(PCB_DEFAULT_TEXT_SIZE_MM),
                hidden: false,
            },
            PcbProperty {
                key: "Value".to_string(),
                value: fp.value.clone(),
                position: Some(Point {
                    x: 0.0,
                    y: PCB_FP_TEXT_OFFSET_MM,
                }),
                rotation: 0.0,
                layer: Some("F.Fab".to_string()),
                font_size: Some(PCB_DEFAULT_TEXT_SIZE_MM),
                hidden: false,
            },
        ];
    }

    let mut properties = fp.properties.clone();
    for property in &mut properties {
        match property.key.as_str() {
            "Reference" => property.value = fp.reference.clone(),
            "Value" => property.value = fp.value.clone(),
            _ => {}
        }
    }

    if !properties.iter().any(|p| p.key == "Reference") {
        properties.insert(
            0,
            PcbProperty {
                key: "Reference".to_string(),
                value: fp.reference.clone(),
                position: Some(Point {
                    x: 0.0,
                    y: -PCB_FP_TEXT_OFFSET_MM,
                }),
                rotation: 0.0,
                layer: Some("F.SilkS".to_string()),
                font_size: Some(PCB_DEFAULT_TEXT_SIZE_MM),
                hidden: false,
            },
        );
    }
    if !properties.iter().any(|p| p.key == "Value") {
        properties.push(PcbProperty {
            key: "Value".to_string(),
            value: fp.value.clone(),
            position: Some(Point {
                x: 0.0,
                y: PCB_FP_TEXT_OFFSET_MM,
            }),
            rotation: 0.0,
            layer: Some("F.Fab".to_string()),
            font_size: Some(PCB_DEFAULT_TEXT_SIZE_MM),
            hidden: false,
        });
    }

    properties
}

fn is_property_backed_text_graphic(g: &FpGraphic, properties: &[PcbProperty]) -> bool {
    if g.graphic_type != "text" {
        return false;
    }
    let Some(position) = g.position else {
        return false;
    };
    properties.iter().filter(|p| !p.hidden).any(|property| {
        let Some(property_pos) = property.position else {
            return false;
        };
        let Some(property_layer) = property.layer.as_deref() else {
            return false;
        };
        let display_text = match property.key.as_str() {
            "Reference" => "%R",
            "Value" => "%V",
            _ => property.value.as_str(),
        };
        let property_font_size = property.font_size.unwrap_or(PCB_DEFAULT_TEXT_SIZE_MM);
        g.layer == property_layer
            && g.text == display_text
            && g.rotation == property.rotation
            && g.font_size == property_font_size
            && position == property_pos
    })
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Serialize a [`PcbBoard`] to the KiCad `.kicad_pcb` S-expression format.
pub fn write_pcb(board: &PcbBoard) -> String {
    let mut children: Vec<SExpr> = vec![
        node("version", [raw(board.version.to_string())]),
        node("generator", [atom("signex")]),
        node("generator_version", [atom("0.1")]),
        build_general(board),
        node("paper", [atom("A4")]),
        build_layers(&board.layers),
    ];

    if let Some(ref setup) = board.setup {
        children.push(build_setup(setup));
        children.push(build_net_class(setup));
    }

    children.extend(board.nets.iter().map(build_net));
    children.extend(board.footprints.iter().map(build_footprint));
    children.extend(board.graphics.iter().filter_map(build_board_graphic));
    children.extend(board.texts.iter().map(board_text_node));
    children.extend(board.segments.iter().map(build_segment));
    children.extend(board.vias.iter().map(build_via));
    children.extend(board.zones.iter().map(build_zone));

    let pcb = node("kicad_pcb", children);
    let mut out = String::with_capacity(64 * 1024);
    write_rendered_sexpr(&mut out, 0, pcb);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr_render::{list, raw};

    fn assert_fragment_matches(actual: &str, expected: SExpr) {
        let parsed = kicad_parser::sexpr::parse(actual).unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn writes_footprint_property_as_expected_sexpr() {
        let property = PcbProperty {
            key: "MPN".to_string(),
            value: "RC0603FR-0710KL".to_string(),
            position: Some(Point { x: 1.0, y: 3.0 }),
            rotation: 180.0,
            layer: Some("Cmts.User".to_string()),
            font_size: Some(1.2),
            hidden: true,
        };
        let mut out = String::new();
        write_rendered_sexpr(&mut out, 4, pcb_property_node(&property));
        assert_fragment_matches(
            out.trim(),
            list(vec![
                raw("property"),
                atom("MPN"),
                atom("RC0603FR-0710KL"),
                list(vec![raw("at"), raw("1"), raw("3"), raw("180")]),
                list(vec![raw("layer"), atom("Cmts.User")]),
                list(vec![raw("hide"), raw("yes")]),
                list(vec![
                    raw("effects"),
                    list(vec![
                        raw("font"),
                        list(vec![raw("size"), raw("1.2"), raw("1.2")]),
                        list(vec![raw("thickness"), raw("0.15")]),
                    ]),
                ]),
            ]),
        );
    }

    #[test]
    fn writes_board_text_as_expected_sexpr() {
        let text = BoardText {
            uuid: Default::default(),
            text: "HELLO".to_string(),
            position: Point { x: 10.0, y: 20.0 },
            rotation: 90.0,
            layer: "F.SilkS".to_string(),
            font_size: 1.5,
        };
        let mut out = String::new();
        write_rendered_sexpr(&mut out, 2, board_text_node(&text));
        assert_fragment_matches(
            out.trim(),
            kicad_parser::sexpr!((
                gr_text "HELLO"
                (at 10 20 90)
                (layer "F.SilkS")
                (effects (font (size 1.5 1.5) (thickness 0.15)))
                (uuid {text.uuid.to_string()})
            )),
        );
    }

    #[test]
    fn writes_structured_footprint_properties_without_duplicate_text_graphics() {
        let fp = Footprint {
            uuid: Default::default(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            footprint_id: "Resistor_SMD:R_0603".to_string(),
            position: Point { x: 10.0, y: 20.0 },
            rotation: 0.0,
            layer: "F.Cu".to_string(),
            locked: false,
            pads: Vec::new(),
            graphics: vec![FpGraphic {
                graphic_type: "text".to_string(),
                layer: "Cmts.User".to_string(),
                width: 0.1,
                start: None,
                end: None,
                center: None,
                mid: None,
                radius: 0.0,
                points: Vec::new(),
                text: "RC0603FR-0710KL".to_string(),
                font_size: 1.2,
                position: Some(Point { x: 1.0, y: 3.0 }),
                rotation: 180.0,
                fill: String::new(),
            }],
            properties: vec![PcbProperty {
                key: "MPN".to_string(),
                value: "RC0603FR-0710KL".to_string(),
                position: Some(Point { x: 1.0, y: 3.0 }),
                rotation: 180.0,
                layer: Some("Cmts.User".to_string()),
                font_size: Some(1.2),
                hidden: false,
            }],
        };

        let mut out = String::new();
        write_rendered_sexpr(&mut out, 2, build_footprint(&fp));

        let parsed = kicad_parser::sexpr::parse(&out).unwrap();
        let property = parsed
            .find_all("property")
            .into_iter()
            .find(|node| node.first_arg() == Some("MPN"))
            .unwrap();
        assert_eq!(property.arg(1), Some("RC0603FR-0710KL"));
        assert_eq!(out.matches("RC0603FR-0710KL").count(), 1);
    }
}
