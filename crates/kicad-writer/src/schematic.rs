use std::fmt::Write;

use signex_types::schematic::*;

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

fn pin_electrical_str(t: PinElectricalType) -> &'static str {
    match t {
        PinElectricalType::Input => "input",
        PinElectricalType::Output => "output",
        PinElectricalType::Bidirectional => "bidirectional",
        PinElectricalType::TriState => "tri_state",
        PinElectricalType::Passive => "passive",
        PinElectricalType::Free => "free",
        PinElectricalType::Unspecified => "unspecified",
        PinElectricalType::PowerIn => "power_in",
        PinElectricalType::PowerOut => "power_out",
        PinElectricalType::OpenCollector => "open_collector",
        PinElectricalType::OpenEmitter => "open_emitter",
        PinElectricalType::NotConnected => "not_connected",
    }
}

fn pin_shape_str(s: PinShape) -> &'static str {
    match s {
        PinShape::Line => "line",
        PinShape::Inverted => "inverted",
        PinShape::Clock => "clock",
        PinShape::InvertedClock => "inverted_clock",
        PinShape::InputLow => "input_low",
        PinShape::ClockLow => "clock_low",
        PinShape::OutputLow => "output_low",
        PinShape::EdgeClockHigh => "edge_clock_high",
        PinShape::NonLogic => "non_logic",
    }
}

fn label_type_keyword(lt: LabelType) -> &'static str {
    match lt {
        LabelType::Net | LabelType::Power => "label",
        LabelType::Global => "global_label",
        LabelType::Hierarchical => "hierarchical_label",
    }
}

// ---------------------------------------------------------------------------
// Float formatting: strip trailing zeros for cleaner output
// ---------------------------------------------------------------------------

fn fmt_f64(v: f64) -> String {
    if v == v.trunc() {
        // Integer value -- emit without decimal cruft
        format!("{}", v as i64)
    } else {
        // Trim trailing zeros
        let s = format!("{:.6}", v);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Serialize a [`SchematicSheet`] to the KiCad `.kicad_sch` S-expression format.
pub fn write_schematic(sheet: &SchematicSheet) -> String {
    let mut out = String::with_capacity(64 * 1024);

    wln!(out, "(kicad_sch");
    wln!(out, "  (version {})", sheet.version);
    wln!(out, "  (generator \"signex\")");
    wln!(out, "  (generator_version \"0.1\")");
    wln!(out, "  (uuid \"{}\")", sheet.uuid);
    wln!(out, "  (paper \"{}\")", sheet.paper_size);

    // Title block
    write_title_block(&mut out, sheet);

    // lib_symbols
    if !sheet.lib_symbols.is_empty() {
        wln!(out, "  (lib_symbols");
        for (id, lib) in &sheet.lib_symbols {
            write_lib_symbol(&mut out, id, lib);
        }
        wln!(out, "  )");
    }

    // Junctions
    for j in &sheet.junctions {
        wln!(out, "  (junction");
        wln!(out, "    (at {} {})", fmt_f64(j.position.x), fmt_f64(j.position.y));
        wln!(out, "    (diameter 0)");
        wln!(out, "    (color 0 0 0 0)");
        wln!(out, "    (uuid \"{}\")", j.uuid);
        wln!(out, "  )");
    }

    // No connects
    for nc in &sheet.no_connects {
        wln!(out, "  (no_connect");
        wln!(out, "    (at {} {})", fmt_f64(nc.position.x), fmt_f64(nc.position.y));
        wln!(out, "    (uuid \"{}\")", nc.uuid);
        wln!(out, "  )");
    }

    // Buses
    for b in &sheet.buses {
        wln!(out, "  (bus");
        wln!(out, "    (pts");
        wln!(
            out,
            "      (xy {} {}) (xy {} {})",
            fmt_f64(b.start.x), fmt_f64(b.start.y),
            fmt_f64(b.end.x), fmt_f64(b.end.y)
        );
        wln!(out, "    )");
        wln!(out, "    (stroke (width 0) (type default) (color 0 0 0 0))");
        wln!(out, "    (uuid \"{}\")", b.uuid);
        wln!(out, "  )");
    }

    // Bus entries
    for be in &sheet.bus_entries {
        wln!(out, "  (bus_entry");
        wln!(out, "    (at {} {})", fmt_f64(be.position.x), fmt_f64(be.position.y));
        wln!(out, "    (size {} {})", fmt_f64(be.size.0), fmt_f64(be.size.1));
        wln!(out, "    (stroke (width 0) (type default) (color 0 0 0 0))");
        wln!(out, "    (uuid \"{}\")", be.uuid);
        wln!(out, "  )");
    }

    // Wires
    for wire in &sheet.wires {
        wln!(out, "  (wire");
        wln!(out, "    (pts");
        wln!(
            out,
            "      (xy {} {}) (xy {} {})",
            fmt_f64(wire.start.x), fmt_f64(wire.start.y),
            fmt_f64(wire.end.x), fmt_f64(wire.end.y)
        );
        wln!(out, "    )");
        wln!(out, "    (stroke");
        wln!(out, "      (width 0)");
        wln!(out, "      (type default)");
        wln!(out, "    )");
        wln!(out, "    (uuid \"{}\")", wire.uuid);
        wln!(out, "  )");
    }

    // Labels
    for l in &sheet.labels {
        write_label(&mut out, l);
    }

    // Symbols (instances)
    for sym in &sheet.symbols {
        write_symbol(&mut out, sym);
    }

    // No ERC directives
    for ne in &sheet.no_erc_directives {
        wln!(out, "  (no_erc");
        wln!(out, "    (at {} {})", fmt_f64(ne.position.x), fmt_f64(ne.position.y));
        wln!(out, "    (uuid \"{}\")", ne.uuid);
        wln!(out, "  )");
    }

    // Text notes
    for note in &sheet.text_notes {
        write_text_note(&mut out, note);
    }

    // Rectangles
    for r in &sheet.rectangles {
        wln!(out, "  (rectangle");
        wln!(out, "    (start {} {})", fmt_f64(r.start.x), fmt_f64(r.start.y));
        wln!(out, "    (end {} {})", fmt_f64(r.end.x), fmt_f64(r.end.y));
        wln!(out, "    (stroke (width 0) (type {}))", escape(&r.stroke_type));
        wln!(out, "    (fill (type none))");
        wln!(out, "    (uuid \"{}\")", r.uuid);
        wln!(out, "  )");
    }

    // Drawing objects
    for d in &sheet.drawings {
        write_drawing(&mut out, d);
    }

    // Child sheets
    for cs in &sheet.child_sheets {
        write_child_sheet(&mut out, cs);
    }

    wln!(out, ")");
    out
}

// ---------------------------------------------------------------------------
// Section writers
// ---------------------------------------------------------------------------

fn write_title_block(out: &mut String, sheet: &SchematicSheet) {
    if sheet.title_block.is_empty() {
        return;
    }
    wln!(out, "  (title_block");
    if let Some(title) = sheet.title_block.get("title") {
        wln!(out, "    (title \"{}\")", escape(title));
    }
    if let Some(date) = sheet.title_block.get("date") {
        wln!(out, "    (date \"{}\")", escape(date));
    }
    if let Some(rev) = sheet.title_block.get("rev") {
        wln!(out, "    (rev \"{}\")", escape(rev));
    }
    if let Some(company) = sheet.title_block.get("company") {
        wln!(out, "    (company \"{}\")", escape(company));
    }
    for i in 1..=9 {
        let key = format!("comment_{}", i);
        if let Some(comment) = sheet.title_block.get(&key) {
            wln!(out, "    (comment {} \"{}\")", i, escape(comment));
        }
    }
    wln!(out, "  )");
}

fn write_label(out: &mut String, l: &Label) {
    let keyword = label_type_keyword(l.label_type);
    wln!(out, "  ({} \"{}\"", keyword, escape(&l.text));
    if !l.shape.is_empty() {
        wln!(out, "    (shape {})", escape(&l.shape));
    }
    wln!(
        out,
        "    (at {} {} {})",
        fmt_f64(l.position.x), fmt_f64(l.position.y), fmt_f64(l.rotation)
    );
    wln!(out, "    (effects");
    wln!(out, "      (font");
    wln!(out, "        (size {} {})", fmt_f64(l.font_size), fmt_f64(l.font_size));
    wln!(out, "      )");
    let needs_justify = l.justify != HAlign::Left
        || matches!(l.label_type, LabelType::Global | LabelType::Hierarchical);
    if needs_justify {
        wln!(out, "      (justify {})", halign_str(l.justify));
    }
    wln!(out, "    )");
    wln!(out, "    (uuid \"{}\")", l.uuid);
    wln!(out, "  )");
}

fn write_symbol(out: &mut String, sym: &Symbol) {
    wln!(out, "  (symbol");
    wln!(out, "    (lib_id \"{}\")", escape(&sym.lib_id));
    wln!(
        out,
        "    (at {} {} {})",
        fmt_f64(sym.position.x), fmt_f64(sym.position.y), fmt_f64(sym.rotation)
    );
    if sym.mirror_x {
        wln!(out, "    (mirror x)");
    }
    if sym.mirror_y {
        wln!(out, "    (mirror y)");
    }
    wln!(out, "    (unit {})", sym.unit);
    if sym.locked {
        wln!(out, "    (locked)");
    }
    wln!(
        out,
        "    (exclude_from_sim {})",
        if sym.exclude_from_sim { "yes" } else { "no" }
    );
    wln!(out, "    (in_bom {})", if sym.in_bom { "yes" } else { "no" });
    wln!(out, "    (on_board {})", if sym.on_board { "yes" } else { "no" });
    wln!(out, "    (dnp {})", if sym.dnp { "yes" } else { "no" });
    if sym.fields_autoplaced {
        wln!(out, "    (fields_autoplaced yes)");
    }
    wln!(out, "    (uuid \"{}\")", sym.uuid);

    // Reference property
    if let Some(ref ref_text) = sym.ref_text {
        write_property(out, "Reference", &sym.reference, ref_text, sym.rotation);
    }
    // Value property
    if let Some(ref val_text) = sym.val_text {
        write_property(out, "Value", &sym.value, val_text, sym.rotation);
    }
    // Footprint property (hidden)
    wln!(
        out,
        "    (property \"Footprint\" \"{}\"",
        escape(&sym.footprint)
    );
    wln!(out, "      (at {} {} 0)", fmt_f64(sym.position.x), fmt_f64(sym.position.y));
    wln!(out, "      (effects (font (size 1.27 1.27)) (hide yes))");
    wln!(out, "    )");

    // Custom fields
    for (key, value) in &sym.fields {
        wln!(out, "    (property \"{}\" \"{}\"", escape(key), escape(value));
        wln!(out, "      (at {} {} 0)", fmt_f64(sym.position.x), fmt_f64(sym.position.y));
        wln!(out, "      (effects (font (size 1.27 1.27)) (hide yes))");
        wln!(out, "    )");
    }

    wln!(out, "  )");
}

fn write_property(out: &mut String, key: &str, value: &str, text: &TextProp, sym_rot: f64) {
    // Reconstruct stored rotation (reverse the toggle applied during parsing)
    let sym_90_270 = (sym_rot - 90.0).abs() < 0.1 || (sym_rot - 270.0).abs() < 0.1;
    let stored_rot = if sym_90_270 {
        if text.rotation.abs() < 0.1 {
            90.0
        } else {
            0.0
        }
    } else {
        text.rotation
    };

    wln!(out, "    (property \"{}\" \"{}\"", key, escape(value));
    wln!(
        out,
        "      (at {} {} {})",
        fmt_f64(text.position.x), fmt_f64(text.position.y), fmt_f64(stored_rot)
    );
    w!(out, "      (effects (font (size {} {}))", fmt_f64(text.font_size), fmt_f64(text.font_size));
    if text.hidden {
        w!(out, " (hide yes)");
    }
    if text.justify_h != HAlign::Center || text.justify_v != VAlign::Center {
        w!(out, " (justify");
        if text.justify_h != HAlign::Center {
            w!(out, " {}", halign_str(text.justify_h));
        }
        if text.justify_v != VAlign::Center {
            w!(out, " {}", valign_str(text.justify_v));
        }
        w!(out, ")");
    }
    wln!(out, ")");
    wln!(out, "    )");
}

fn write_text_note(out: &mut String, note: &TextNote) {
    wln!(
        out,
        "  (text \"{}\"",
        escape(&note.text.replace('\n', "\\n"))
    );
    wln!(out, "    (exclude_from_sim no)");
    wln!(
        out,
        "    (at {} {} {})",
        fmt_f64(note.position.x), fmt_f64(note.position.y), fmt_f64(note.rotation)
    );
    wln!(out, "    (effects");
    wln!(out, "      (font");
    wln!(out, "        (size {} {})", fmt_f64(note.font_size), fmt_f64(note.font_size));
    wln!(out, "      )");
    wln!(out, "    )");
    wln!(out, "    (uuid \"{}\")", note.uuid);
    wln!(out, "  )");
}

fn write_drawing(out: &mut String, d: &SchDrawing) {
    match d {
        SchDrawing::Line { uuid, start, end, width } => {
            wln!(out, "  (polyline");
            wln!(
                out,
                "    (pts (xy {} {}) (xy {} {}))",
                fmt_f64(start.x), fmt_f64(start.y), fmt_f64(end.x), fmt_f64(end.y)
            );
            wln!(out, "    (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Polyline { uuid, points, width, fill } => {
            wln!(out, "  (polyline");
            w!(out, "    (pts");
            for p in points {
                w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
            }
            wln!(out, ")");
            wln!(out, "    (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(
                out,
                "    (fill (type {}))",
                if *fill { "outline" } else { "none" }
            );
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Circle { uuid, center, radius, width, fill } => {
            wln!(out, "  (circle");
            wln!(out, "    (center {} {})", fmt_f64(center.x), fmt_f64(center.y));
            wln!(out, "    (radius {})", fmt_f64(*radius));
            wln!(out, "    (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Arc { uuid, start, mid, end, width } => {
            wln!(out, "  (arc");
            wln!(out, "    (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "    (mid {} {})", fmt_f64(mid.x), fmt_f64(mid.y));
            wln!(out, "    (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(out, "    (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Rect { uuid, start, end, width, fill } => {
            wln!(out, "  (rectangle");
            wln!(out, "    (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "    (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(out, "    (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
    }
}

fn write_child_sheet(out: &mut String, cs: &ChildSheet) {
    wln!(out, "  (sheet");
    wln!(out, "    (at {} {})", fmt_f64(cs.position.x), fmt_f64(cs.position.y));
    wln!(out, "    (size {} {})", fmt_f64(cs.size.0), fmt_f64(cs.size.1));
    wln!(out, "    (uuid \"{}\")", cs.uuid);
    // Sheetname property
    wln!(out, "    (property \"Sheetname\" \"{}\"", escape(&cs.name));
    wln!(
        out,
        "      (at {} {} 0)",
        fmt_f64(cs.position.x),
        fmt_f64(cs.position.y - 1.0)
    );
    wln!(out, "      (effects (font (size 1.27 1.27)) (justify left bottom))");
    wln!(out, "    )");
    // Sheetfile property
    wln!(out, "    (property \"Sheetfile\" \"{}\"", escape(&cs.filename));
    wln!(
        out,
        "      (at {} {} 0)",
        fmt_f64(cs.position.x),
        fmt_f64(cs.position.y + cs.size.1 + 1.0)
    );
    wln!(out, "      (effects (font (size 1.27 1.27)) (justify left top))");
    wln!(out, "    )");
    // Sheet pins
    for pin in &cs.pins {
        wln!(out, "    (pin \"{}\" {}", escape(&pin.name), pin.direction);
        wln!(
            out,
            "      (at {} {} {})",
            fmt_f64(pin.position.x), fmt_f64(pin.position.y), fmt_f64(pin.rotation)
        );
        wln!(out, "      (effects (font (size 1.27 1.27)) (justify left))");
        wln!(out, "      (uuid \"{}\")", pin.uuid);
        wln!(out, "    )");
    }
    wln!(out, "  )");
}

// ---------------------------------------------------------------------------
// lib_symbol writer
// ---------------------------------------------------------------------------

fn write_lib_symbol(out: &mut String, _id: &str, lib: &LibSymbol) {
    wln!(out, "    (symbol \"{}\"", escape(&lib.id));
    if !lib.show_pin_numbers {
        wln!(out, "      (pin_numbers hide)");
    }
    if !lib.show_pin_names {
        wln!(
            out,
            "      (pin_names (offset {}) hide)",
            fmt_f64(lib.pin_name_offset)
        );
    } else {
        wln!(out, "      (pin_names (offset {}))", fmt_f64(lib.pin_name_offset));
    }

    // Sub-symbol for graphics
    let base_name = lib.id.split(':').next_back().unwrap_or(&lib.id);
    wln!(out, "      (symbol \"{}_0_1\"", base_name);
    for g in &lib.graphics {
        write_lib_graphic(out, g);
    }
    wln!(out, "      )");

    // Sub-symbol for pins
    wln!(out, "      (symbol \"{}_1_1\"", base_name);
    for pin in &lib.pins {
        write_lib_pin(out, pin);
    }
    wln!(out, "      )");

    wln!(out, "    )");
}

fn write_lib_graphic(out: &mut String, g: &Graphic) {
    match g {
        Graphic::Polyline { points, width, fill } => {
            wln!(out, "        (polyline");
            w!(out, "          (pts");
            for p in points {
                w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
            }
            wln!(out, ")");
            wln!(out, "          (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Rectangle { start, end, width, fill } => {
            wln!(out, "        (rectangle");
            wln!(out, "          (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "          (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(out, "          (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Circle { center, radius, width, fill } => {
            wln!(out, "        (circle");
            wln!(out, "          (center {} {})", fmt_f64(center.x), fmt_f64(center.y));
            wln!(out, "          (radius {})", fmt_f64(*radius));
            wln!(out, "          (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Arc { start, mid, end, width, fill } => {
            wln!(out, "        (arc");
            wln!(out, "          (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "          (mid {} {})", fmt_f64(mid.x), fmt_f64(mid.y));
            wln!(out, "          (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(out, "          (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Text { text, position, rotation, font_size, bold, italic, justify_h, justify_v } => {
            wln!(out, "        (text {:?}", text);
            wln!(
                out,
                "          (at {} {} {})",
                fmt_f64(position.x), fmt_f64(position.y), fmt_f64(*rotation)
            );
            w!(out, "          (effects (font (size {} {})", fmt_f64(*font_size), fmt_f64(*font_size));
            if *bold {
                w!(out, " (bold yes)");
            }
            if *italic {
                w!(out, " (italic yes)");
            }
            w!(out, ")");
            if *justify_h != HAlign::Center || *justify_v != VAlign::Center {
                w!(out, " (justify");
                if *justify_h != HAlign::Center {
                    w!(out, " {}", halign_str(*justify_h));
                }
                if *justify_v != VAlign::Center {
                    w!(out, " {}", valign_str(*justify_v));
                }
                w!(out, ")");
            }
            wln!(out, ")");
            wln!(out, "        )");
        }
        Graphic::TextBox { text, position, rotation, size, font_size, bold, italic, width, fill } => {
            wln!(out, "        (text_box {:?}", text);
            wln!(
                out,
                "          (at {} {} {})",
                fmt_f64(position.x), fmt_f64(position.y), fmt_f64(*rotation)
            );
            wln!(out, "          (size {} {})", fmt_f64(size.x), fmt_f64(size.y));
            wln!(out, "          (stroke (width {}) (type default))", fmt_f64(*width));
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            w!(out, "          (effects (font (size {} {})", fmt_f64(*font_size), fmt_f64(*font_size));
            if *bold {
                w!(out, " (bold yes)");
            }
            if *italic {
                w!(out, " (italic yes)");
            }
            wln!(out, "))");
            wln!(out, "        )");
        }
    }
}

fn write_lib_pin(out: &mut String, pin: &Pin) {
    wln!(
        out,
        "        (pin {} {}",
        pin_electrical_str(pin.pin_type),
        pin_shape_str(pin.shape)
    );
    wln!(
        out,
        "          (at {} {} {})",
        fmt_f64(pin.position.x), fmt_f64(pin.position.y), fmt_f64(pin.rotation)
    );
    wln!(out, "          (length {})", fmt_f64(pin.length));
    wln!(
        out,
        "          (name \"{}\" (effects (font (size 1.27 1.27))))",
        escape(&pin.name)
    );
    wln!(
        out,
        "          (number \"{}\" (effects (font (size 1.27 1.27))))",
        escape(&pin.number)
    );
    wln!(out, "        )");
}
