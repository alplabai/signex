use std::fmt::Write;

use signex_types::property::PcbProperty;
use signex_types::pcb::*;

use crate::sexpr_render::{
    at_node, atom, effects_node, hide_yes_node, node, write_rendered_sexpr, SExpr,
};

// String's Write impl is infallible -- these macros avoid many `.unwrap()` calls.
macro_rules! w {
    ($dst:expr, $($arg:tt)*) => { let _ = write!($dst, $($arg)*); };
}
macro_rules! wln {
    ($dst:expr, $($arg:tt)*) => { let _ = writeln!($dst, $($arg)*); };
}

// ---------------------------------------------------------------------------
// KiCad S-expression string escaping
// ---------------------------------------------------------------------------

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

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

fn pcb_text_effects_node(font_size: f64) -> SExpr {
    effects_node(font_size, Some(0.15), false, false, Vec::new())
}

fn pcb_property_node(property: &PcbProperty) -> SExpr {
    let position = property.position.unwrap_or(Point { x: 0.0, y: 0.0 });
    let mut items = vec![atom(&property.key), atom(&property.value)];

    items.push(at_node(
        position.x,
        position.y,
        (property.rotation != 0.0).then_some(property.rotation),
    ));

    if let Some(layer) = &property.layer {
        items.push(node("layer", vec![atom(layer)]));
    }
    if property.hidden {
        items.push(hide_yes_node());
    }
    items.push(pcb_text_effects_node(property.font_size.unwrap_or(1.0)));
    node("property", items)
}

fn board_text_node(text: &BoardText) -> SExpr {
    let mut items = vec![atom(&text.text)];
    items.push(at_node(
        text.position.x,
        text.position.y,
        (text.rotation != 0.0).then_some(text.rotation),
    ));
    items.push(node("layer", vec![atom(&text.layer)]));
    items.push(pcb_text_effects_node(text.font_size));
    items.push(node("uuid", vec![atom(text.uuid.to_string())]));
    node("gr_text", items)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Serialize a [`PcbBoard`] to the KiCad `.kicad_pcb` S-expression format.
pub fn write_pcb(board: &PcbBoard) -> String {
    let mut out = String::with_capacity(64 * 1024);

    wln!(out, "(kicad_pcb");
    wln!(out, "  (version {})", board.version);
    wln!(out, "  (generator \"signex\")");
    wln!(out, "  (generator_version \"0.1\")");
    wln!(out, "  (general");
    wln!(out, "    (thickness {})", fmt_f64(board.thickness));
    wln!(out, "    (uuid \"{}\")", board.uuid);
    wln!(out, "  )");

    // Paper size
    wln!(out, "  (paper \"A4\")");

    // Layers
    write_layers(&mut out, &board.layers);

    // Setup
    if let Some(ref setup) = board.setup {
        write_setup(&mut out, setup);
    }

    // Nets
    for net in &board.nets {
        wln!(out, "  (net {} \"{}\")", net.number, escape(&net.name));
    }

    // Footprints
    for fp in &board.footprints {
        write_footprint(&mut out, fp);
    }

    // Board-level graphics
    for g in &board.graphics {
        write_board_graphic(&mut out, g);
    }

    // Board-level texts
    for t in &board.texts {
        write_board_text(&mut out, t);
    }

    // Segments (traces)
    for seg in &board.segments {
        wln!(out, "  (segment");
        wln!(
            out,
            "    (start {} {})",
            fmt_f64(seg.start.x),
            fmt_f64(seg.start.y)
        );
        wln!(
            out,
            "    (end {} {})",
            fmt_f64(seg.end.x),
            fmt_f64(seg.end.y)
        );
        wln!(out, "    (width {})", fmt_f64(seg.width));
        wln!(out, "    (layer \"{}\")", escape(&seg.layer));
        wln!(out, "    (net {})", seg.net);
        wln!(out, "    (uuid \"{}\")", seg.uuid);
        wln!(out, "  )");
    }

    // Vias
    for v in &board.vias {
        write_via(&mut out, v);
    }

    // Zones
    for z in &board.zones {
        write_zone(&mut out, z);
    }

    wln!(out, ")");
    out
}

// ---------------------------------------------------------------------------
// Section writers
// ---------------------------------------------------------------------------

fn write_layers(out: &mut String, layers: &[LayerDef]) {
    wln!(out, "  (layers");
    for l in layers {
        wln!(
            out,
            "    ({} \"{}\" {})",
            l.id,
            escape(&l.name),
            escape(&l.layer_type)
        );
    }
    wln!(out, "  )");
}

fn write_setup(out: &mut String, setup: &PcbSetup) {
    wln!(out, "  (setup");
    wln!(out, "    (pad_to_mask_clearance 0)");
    wln!(out, "    (pcbplotparams");
    wln!(out, "      (layerselection 0x00010fc_ffffffff)");
    wln!(
        out,
        "      (plot_on_all_layers_selection 0x0000000_00000000)"
    );
    wln!(out, "    )");
    wln!(out, "  )");
    // Net classes with defaults from setup
    wln!(out, "  (net_class \"Default\" \"\"");
    wln!(out, "    (clearance {})", fmt_f64(setup.clearance));
    wln!(out, "    (trace_width {})", fmt_f64(setup.trace_width));
    wln!(out, "    (via_dia {})", fmt_f64(setup.via_diameter));
    wln!(out, "    (via_drill {})", fmt_f64(setup.via_drill));
    wln!(out, "    (uvia_dia {})", fmt_f64(setup.via_min_diameter));
    wln!(out, "    (uvia_drill {})", fmt_f64(setup.via_min_drill));
    wln!(out, "  )");
}

fn write_footprint(out: &mut String, fp: &Footprint) {
    wln!(out, "  (footprint \"{}\"", escape(&fp.footprint_id));
    if fp.locked {
        wln!(out, "    (locked yes)");
    }
    wln!(out, "    (layer \"{}\")", escape(&fp.layer));
    if fp.rotation != 0.0 {
        wln!(
            out,
            "    (at {} {} {})",
            fmt_f64(fp.position.x),
            fmt_f64(fp.position.y),
            fmt_f64(fp.rotation)
        );
    } else {
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(fp.position.x),
            fmt_f64(fp.position.y)
        );
    }
    wln!(out, "    (uuid \"{}\")", fp.uuid);

    let properties = effective_footprint_properties(fp);
    for property in &properties {
        write_footprint_property(out, property);
    }

    // Footprint graphics
    for g in &fp.graphics {
        if is_property_backed_text_graphic(g, &properties) {
            continue;
        }
        write_fp_graphic(out, g);
    }

    // Pads
    for p in &fp.pads {
        write_fp_pad(out, p);
    }

    wln!(out, "  )");
}

fn effective_footprint_properties(fp: &Footprint) -> Vec<PcbProperty> {
    if fp.properties.is_empty() {
        return vec![
            PcbProperty {
                key: "Reference".to_string(),
                value: fp.reference.clone(),
                position: Some(Point { x: 0.0, y: -2.0 }),
                rotation: 0.0,
                layer: Some("F.SilkS".to_string()),
                font_size: Some(1.0),
                hidden: false,
            },
            PcbProperty {
                key: "Value".to_string(),
                value: fp.value.clone(),
                position: Some(Point { x: 0.0, y: 2.0 }),
                rotation: 0.0,
                layer: Some("F.Fab".to_string()),
                font_size: Some(1.0),
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

    if !properties.iter().any(|property| property.key == "Reference") {
        properties.insert(
            0,
            PcbProperty {
                key: "Reference".to_string(),
                value: fp.reference.clone(),
                position: Some(Point { x: 0.0, y: -2.0 }),
                rotation: 0.0,
                layer: Some("F.SilkS".to_string()),
                font_size: Some(1.0),
                hidden: false,
            },
        );
    }
    if !properties.iter().any(|property| property.key == "Value") {
        properties.push(PcbProperty {
            key: "Value".to_string(),
            value: fp.value.clone(),
            position: Some(Point { x: 0.0, y: 2.0 }),
            rotation: 0.0,
            layer: Some("F.Fab".to_string()),
            font_size: Some(1.0),
            hidden: false,
        });
    }

    properties
}

fn write_footprint_property(out: &mut String, property: &PcbProperty) {
    write_rendered_sexpr(out, 4, pcb_property_node(property));
}

fn is_property_backed_text_graphic(g: &FpGraphic, properties: &[PcbProperty]) -> bool {
    if g.graphic_type != "text" {
        return false;
    }

    let Some(position) = g.position else {
        return false;
    };

    properties.iter().filter(|property| !property.hidden).any(|property| {
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
        let property_font_size = property.font_size.unwrap_or(1.0);

        g.layer == property_layer
            && g.text == display_text
            && g.rotation == property.rotation
            && g.font_size == property_font_size
            && position == property_pos
    })
}

fn write_fp_graphic(out: &mut String, g: &FpGraphic) {
    match g.graphic_type.as_str() {
        "line" => {
            if let (Some(s), Some(e)) = (&g.start, &g.end) {
                wln!(out, "    (fp_line");
                wln!(out, "      (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "      (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "      (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(out, "    )");
            }
        }
        "rect" => {
            if let (Some(s), Some(e)) = (&g.start, &g.end) {
                wln!(out, "    (fp_rect");
                wln!(out, "      (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "      (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "      (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                if g.fill == "solid" {
                    wln!(out, "      (fill solid)");
                }
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(out, "    )");
            }
        }
        "circle" => {
            if let Some(c) = &g.center {
                let r = g.radius;
                wln!(out, "    (fp_circle");
                wln!(out, "      (center {} {})", fmt_f64(c.x), fmt_f64(c.y));
                wln!(out, "      (end {} {})", fmt_f64(c.x + r), fmt_f64(c.y));
                wln!(
                    out,
                    "      (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                if g.fill == "solid" {
                    wln!(out, "      (fill solid)");
                }
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(out, "    )");
            }
        }
        "arc" => {
            if let (Some(s), Some(m), Some(e)) = (&g.start, &g.mid, &g.end) {
                wln!(out, "    (fp_arc");
                wln!(out, "      (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "      (mid {} {})", fmt_f64(m.x), fmt_f64(m.y));
                wln!(out, "      (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "      (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(out, "    )");
            }
        }
        "poly" => {
            if g.points.len() >= 2 {
                wln!(out, "    (fp_poly");
                w!(out, "      (pts");
                for p in &g.points {
                    w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
                }
                wln!(out, ")");
                wln!(
                    out,
                    "      (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                if g.fill == "solid" {
                    wln!(out, "      (fill solid)");
                }
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(out, "    )");
            }
        }
        "text" => {
            if let Some(pos) = &g.position {
                let fs = if g.font_size != 0.0 { g.font_size } else { 1.0 };
                wln!(out, "    (fp_text user \"{}\"", escape(&g.text));
                if g.rotation != 0.0 {
                    wln!(
                        out,
                        "      (at {} {} {})",
                        fmt_f64(pos.x),
                        fmt_f64(pos.y),
                        fmt_f64(g.rotation)
                    );
                } else {
                    wln!(out, "      (at {} {})", fmt_f64(pos.x), fmt_f64(pos.y));
                }
                wln!(out, "      (layer \"{}\")", escape(&g.layer));
                wln!(
                    out,
                    "      (effects (font (size {} {}) (thickness 0.15)))",
                    fmt_f64(fs),
                    fmt_f64(fs)
                );
                wln!(out, "    )");
            }
        }
        _ => {}
    }
}

fn write_fp_pad(out: &mut String, p: &Pad) {
    w!(
        out,
        "    (pad \"{}\" {} {}",
        escape(&p.number),
        pad_type_str(p.pad_type),
        pad_shape_str(p.shape)
    );
    wln!(out, "");
    wln!(
        out,
        "      (at {} {})",
        fmt_f64(p.position.x),
        fmt_f64(p.position.y)
    );
    wln!(
        out,
        "      (size {} {})",
        fmt_f64(p.size.x),
        fmt_f64(p.size.y)
    );

    if let Some(ref drill) = p.drill {
        if !drill.shape.is_empty() {
            wln!(
                out,
                "      (drill {} {})",
                escape(&drill.shape),
                fmt_f64(drill.diameter)
            );
        } else {
            wln!(out, "      (drill {})", fmt_f64(drill.diameter));
        }
    }

    w!(out, "      (layers");
    for l in &p.layers {
        w!(out, " \"{}\"", escape(l));
    }
    wln!(out, ")");

    if p.roundrect_ratio != 0.0 {
        wln!(
            out,
            "      (roundrect_rratio {})",
            fmt_f64(p.roundrect_ratio)
        );
    }

    if let Some(ref net) = p.net {
        wln!(out, "      (net {} \"{}\")", net.number, escape(&net.name));
    }

    wln!(out, "      (uuid \"{}\")", p.uuid);
    wln!(out, "    )");
}

fn write_via(out: &mut String, v: &Via) {
    let kw = via_type_str(v.via_type);
    wln!(out, "  ({}", kw);
    wln!(
        out,
        "    (at {} {})",
        fmt_f64(v.position.x),
        fmt_f64(v.position.y)
    );
    wln!(out, "    (size {})", fmt_f64(v.diameter));
    wln!(out, "    (drill {})", fmt_f64(v.drill));
    if v.layers.len() >= 2 {
        wln!(
            out,
            "    (layers \"{}\" \"{}\")",
            escape(&v.layers[0]),
            escape(&v.layers[1])
        );
    }
    wln!(out, "    (net {})", v.net);
    wln!(out, "    (uuid \"{}\")", v.uuid);
    wln!(out, "  )");
}

fn write_zone(out: &mut String, z: &Zone) {
    wln!(out, "  (zone");
    wln!(out, "    (net {})", z.net);
    wln!(out, "    (net_name \"{}\")", escape(&z.net_name));
    wln!(out, "    (layer \"{}\")", escape(&z.layer));
    wln!(out, "    (uuid \"{}\")", z.uuid);
    if z.priority > 0 {
        wln!(out, "    (priority {})", z.priority);
    }
    // Fill settings
    wln!(out, "    (fill");
    if z.thermal_relief {
        wln!(out, "      (thermal_relief)");
        wln!(out, "      (thermal_gap {})", fmt_f64(z.thermal_gap));
        wln!(
            out,
            "      (thermal_bridge_width {})",
            fmt_f64(z.thermal_width)
        );
    }
    wln!(out, "    )");
    wln!(out, "    (min_thickness {})", fmt_f64(z.min_thickness));
    if z.clearance > 0.0 {
        wln!(out, "    (clearance {})", fmt_f64(z.clearance));
    }
    // Polygon outline
    if !z.outline.is_empty() {
        wln!(out, "    (polygon");
        w!(out, "      (pts");
        for p in &z.outline {
            w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
        }
        wln!(out, ")");
        wln!(out, "    )");
    }
    wln!(out, "  )");
}

fn write_board_graphic(out: &mut String, g: &BoardGraphic) {
    match g.graphic_type.as_str() {
        "line" => {
            if let (Some(s), Some(e)) = (&g.start, &g.end) {
                wln!(out, "  (gr_line");
                wln!(out, "    (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "    (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "    (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "    (layer \"{}\")", escape(&g.layer));
                wln!(out, "  )");
            }
        }
        "rect" => {
            if let (Some(s), Some(e)) = (&g.start, &g.end) {
                wln!(out, "  (gr_rect");
                wln!(out, "    (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "    (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "    (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "    (layer \"{}\")", escape(&g.layer));
                wln!(out, "  )");
            }
        }
        "circle" => {
            if let Some(c) = &g.center {
                let r = g.radius;
                wln!(out, "  (gr_circle");
                wln!(out, "    (center {} {})", fmt_f64(c.x), fmt_f64(c.y));
                wln!(out, "    (end {} {})", fmt_f64(c.x + r), fmt_f64(c.y));
                wln!(
                    out,
                    "    (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "    (layer \"{}\")", escape(&g.layer));
                wln!(out, "  )");
            }
        }
        "arc" => {
            if let (Some(s), Some(e)) = (&g.start, &g.end) {
                wln!(out, "  (gr_arc");
                wln!(out, "    (start {} {})", fmt_f64(s.x), fmt_f64(s.y));
                wln!(out, "    (end {} {})", fmt_f64(e.x), fmt_f64(e.y));
                wln!(
                    out,
                    "    (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "    (layer \"{}\")", escape(&g.layer));
                wln!(out, "  )");
            }
        }
        "poly" => {
            if g.points.len() >= 2 {
                wln!(out, "  (gr_poly");
                w!(out, "    (pts");
                for p in &g.points {
                    w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
                }
                wln!(out, ")");
                wln!(
                    out,
                    "    (stroke (width {}) (type default))",
                    fmt_f64(g.width)
                );
                wln!(out, "    (layer \"{}\")", escape(&g.layer));
                wln!(out, "  )");
            }
        }
        _ => {}
    }
}

fn write_board_text(out: &mut String, t: &BoardText) {
    write_rendered_sexpr(out, 2, board_text_node(t));
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
        let mut out = String::new();
        let property = PcbProperty {
            key: "MPN".to_string(),
            value: "RC0603FR-0710KL".to_string(),
            position: Some(Point { x: 1.0, y: 3.0 }),
            rotation: 180.0,
            layer: Some("Cmts.User".to_string()),
            font_size: Some(1.2),
            hidden: true,
        };

        write_footprint_property(&mut out, &property);

        assert_fragment_matches(
            out.trim(),
            list(vec![
                raw("property"),
                atom("MPN"),
                atom("RC0603FR-0710KL"),
                list(vec![raw("at"), atom(1.0_f64), atom(3.0_f64), atom(180.0_f64)]),
                list(vec![raw("layer"), atom("Cmts.User")]),
                list(vec![raw("hide"), raw("yes")]),
                list(vec![
                    raw("effects"),
                    list(vec![
                        raw("font"),
                        list(vec![raw("size"), atom(1.2_f64), atom(1.2_f64)]),
                        list(vec![raw("thickness"), atom(0.15_f64)]),
                    ]),
                ]),
            ]),
        );
    }

    #[test]
    fn writes_board_text_as_expected_sexpr() {
        let mut out = String::new();
        let text = BoardText {
            uuid: Default::default(),
            text: "HELLO".to_string(),
            position: Point { x: 10.0, y: 20.0 },
            rotation: 90.0,
            layer: "F.SilkS".to_string(),
            font_size: 1.5,
        };

        write_board_text(&mut out, &text);

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
        write_footprint(&mut out, &fp);

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
