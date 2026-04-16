use std::fmt::Write;

use signex_types::pcb::*;

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

    // Reference property
    wln!(
        out,
        "    (property \"Reference\" \"{}\"",
        escape(&fp.reference)
    );
    wln!(out, "      (at 0 -2)");
    wln!(out, "      (layer \"F.SilkS\")");
    wln!(out, "      (effects (font (size 1 1) (thickness 0.15)))");
    wln!(out, "    )");

    // Value property
    wln!(out, "    (property \"Value\" \"{}\"", escape(&fp.value));
    wln!(out, "      (at 0 2)");
    wln!(out, "      (layer \"F.Fab\")");
    wln!(out, "      (effects (font (size 1 1) (thickness 0.15)))");
    wln!(out, "    )");

    // Footprint graphics
    for g in &fp.graphics {
        write_fp_graphic(out, g);
    }

    // Pads
    for p in &fp.pads {
        write_fp_pad(out, p);
    }

    wln!(out, "  )");
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
    wln!(out, "  (gr_text \"{}\"", escape(&t.text));
    if t.rotation != 0.0 {
        wln!(
            out,
            "    (at {} {} {})",
            fmt_f64(t.position.x),
            fmt_f64(t.position.y),
            fmt_f64(t.rotation)
        );
    } else {
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(t.position.x),
            fmt_f64(t.position.y)
        );
    }
    wln!(out, "    (layer \"{}\")", escape(&t.layer));
    wln!(
        out,
        "    (effects (font (size {} {}) (thickness 0.15)))",
        fmt_f64(t.font_size),
        fmt_f64(t.font_size)
    );
    wln!(out, "    (uuid \"{}\")", t.uuid);
    wln!(out, "  )");
}
