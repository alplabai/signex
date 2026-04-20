use std::{collections::BTreeMap, fmt::Write};

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
        PinElectricalType::NotConnected => "no_connect",
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

    // lib_symbols — sort keys for deterministic output order
    if !sheet.lib_symbols.is_empty() {
        wln!(out, "  (lib_symbols");
        let mut lib_ids: Vec<_> = sheet.lib_symbols.keys().collect();
        lib_ids.sort();
        for id in lib_ids {
            let lib = &sheet.lib_symbols[id];
            write_lib_symbol(&mut out, id, lib);
        }
        wln!(out, "  )");
    }

    // Junctions
    for j in &sheet.junctions {
        wln!(out, "  (junction");
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(j.position.x),
            fmt_f64(j.position.y)
        );
        wln!(out, "    (diameter 0)");
        wln!(out, "    (color 0 0 0 0)");
        wln!(out, "    (uuid \"{}\")", j.uuid);
        wln!(out, "  )");
    }

    // No connects
    for nc in &sheet.no_connects {
        wln!(out, "  (no_connect");
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(nc.position.x),
            fmt_f64(nc.position.y)
        );
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
            fmt_f64(b.start.x),
            fmt_f64(b.start.y),
            fmt_f64(b.end.x),
            fmt_f64(b.end.y)
        );
        wln!(out, "    )");
        wln!(out, "    (stroke (width 0) (type default) (color 0 0 0 0))");
        wln!(out, "    (uuid \"{}\")", b.uuid);
        wln!(out, "  )");
    }

    // Bus entries
    for be in &sheet.bus_entries {
        wln!(out, "  (bus_entry");
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(be.position.x),
            fmt_f64(be.position.y)
        );
        wln!(
            out,
            "    (size {} {})",
            fmt_f64(be.size.0),
            fmt_f64(be.size.1)
        );
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
            fmt_f64(wire.start.x),
            fmt_f64(wire.start.y),
            fmt_f64(wire.end.x),
            fmt_f64(wire.end.y)
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
        wln!(
            out,
            "    (at {} {})",
            fmt_f64(ne.position.x),
            fmt_f64(ne.position.y)
        );
        wln!(out, "    (uuid \"{}\")", ne.uuid);
        wln!(out, "  )");
    }

    // Text notes
    for note in &sheet.text_notes {
        write_text_note(&mut out, note);
    }

    // Drawing objects
    for d in &sheet.drawings {
        write_drawing(&mut out, d);
    }

    // Child sheets
    for cs in &sheet.child_sheets {
        write_child_sheet(&mut out, cs);
    }

    wln!(out, "  (sheet_instances");
    wln!(out, "    (path \"/\"");
    wln!(out, "      (page \"{}\")", escape(&sheet.root_sheet_page));
    wln!(out, "    )");
    wln!(out, "  )");

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
        fmt_f64(l.position.x),
        fmt_f64(l.position.y),
        fmt_f64(l.rotation)
    );
    wln!(out, "    (effects");
    wln!(out, "      (font");
    wln!(
        out,
        "        (size {} {})",
        fmt_f64(l.font_size),
        fmt_f64(l.font_size)
    );
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
        fmt_f64(sym.position.x),
        fmt_f64(sym.position.y),
        fmt_f64(sym.rotation)
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
    wln!(
        out,
        "    (in_bom {})",
        if sym.in_bom { "yes" } else { "no" }
    );
    wln!(
        out,
        "    (on_board {})",
        if sym.on_board { "yes" } else { "no" }
    );
    wln!(out, "    (dnp {})", if sym.dnp { "yes" } else { "no" });
    if sym.fields_autoplaced {
        wln!(out, "    (fields_autoplaced)");
    }
    wln!(out, "    (uuid \"{}\")", sym.uuid);

    // Reference property — always written; hidden when ref_text is None (power symbols).
    match sym.ref_text.as_ref() {
        Some(ref_text) => write_property(out, "Reference", &sym.reference, ref_text, sym.rotation),
        None => {
            wln!(
                out,
                "    (property \"Reference\" \"{}\"",
                escape(&sym.reference)
            );
            wln!(
                out,
                "      (at {} {} 0)",
                fmt_f64(sym.position.x),
                fmt_f64(sym.position.y)
            );
            wln!(out, "      (show_name no)");
            wln!(out, "      (do_not_autoplace no)");
            wln!(out, "      (hide yes)");
            wln!(out, "      (effects (font (size 1.27 1.27)))");
            wln!(out, "    )");
        }
    }
    // Value property — always written; hidden when val_text is None.
    match sym.val_text.as_ref() {
        Some(val_text) => write_property(out, "Value", &sym.value, val_text, sym.rotation),
        None => {
            wln!(out, "    (property \"Value\" \"{}\"", escape(&sym.value));
            wln!(
                out,
                "      (at {} {} 0)",
                fmt_f64(sym.position.x),
                fmt_f64(sym.position.y)
            );
            wln!(out, "      (show_name no)");
            wln!(out, "      (do_not_autoplace no)");
            wln!(out, "      (effects (font (size 1.27 1.27)))");
            wln!(out, "    )");
        }
    }
    // Footprint and Datasheet: always hidden at property level (KiCad 8 format).
    wln!(out, "    (property \"Footprint\" \"{}\"", escape(&sym.footprint));
    wln!(out, "      (at {} {} 0)", fmt_f64(sym.position.x), fmt_f64(sym.position.y));
    wln!(out, "      (show_name no)");
    wln!(out, "      (do_not_autoplace no)");
    wln!(out, "      (hide yes)");
    wln!(out, "      (effects (font (size 1.27 1.27)))");
    wln!(out, "    )");

    wln!(out, "    (property \"Datasheet\" \"{}\"", escape(&sym.datasheet));
    wln!(out, "      (at {} {} 0)", fmt_f64(sym.position.x), fmt_f64(sym.position.y));
    wln!(out, "      (show_name no)");
    wln!(out, "      (do_not_autoplace no)");
    wln!(out, "      (hide yes)");
    wln!(out, "      (effects (font (size 1.27 1.27)))");
    wln!(out, "    )");

    // Custom fields — sort keys for deterministic output order
    let mut field_keys: Vec<_> = sym.fields.keys().collect();
    field_keys.sort();
    for key in field_keys {
        let value = &sym.fields[key];
        wln!(
            out,
            "    (property \"{}\" \"{}\"",
            escape(key),
            escape(value)
        );
        wln!(
            out,
            "      (at {} {} 0)",
            fmt_f64(sym.position.x),
            fmt_f64(sym.position.y)
        );
        wln!(out, "      (show_name no)");
        wln!(out, "      (do_not_autoplace no)");
        wln!(out, "      (effects (font (size 1.27 1.27)) (hide yes))");
        wln!(out, "    )");
    }

    let mut pin_entries: Vec<_> = sym.pin_uuids.iter().collect();
    pin_entries.sort_by(|left, right| left.0.cmp(right.0));
    for (pin_number, pin_uuid) in pin_entries {
        wln!(
            out,
            "    (pin \"{}\" (uuid \"{}\"))",
            escape(pin_number),
            pin_uuid
        );
    }

    write_symbol_instances(out, &sym.instances);

    wln!(out, "  )");
}

fn write_symbol_instances(out: &mut String, instances: &[SymbolInstance]) {
    if instances.is_empty() {
        return;
    }

    let mut grouped: BTreeMap<&str, Vec<&SymbolInstance>> = BTreeMap::new();
    for instance in instances {
        grouped
            .entry(instance.project.as_str())
            .or_default()
            .push(instance);
    }

    wln!(out, "    (instances");
    for (project, project_instances) in grouped {
        wln!(out, "      (project \"{}\"", escape(project));
        let mut sorted_instances = project_instances;
        sorted_instances.sort_by(|left, right| left.path.cmp(&right.path));
        for instance in sorted_instances {
            wln!(out, "        (path \"{}\"", escape(&instance.path));
            wln!(
                out,
                "          (reference \"{}\")",
                escape(&instance.reference)
            );
            wln!(out, "          (unit {})", instance.unit);
            wln!(out, "        )");
        }
        wln!(out, "      )");
    }
    wln!(out, "    )");
}

fn write_sheet_instances(out: &mut String, instances: &[SheetInstance]) {
    if instances.is_empty() {
        return;
    }

    let mut grouped: BTreeMap<&str, Vec<&SheetInstance>> = BTreeMap::new();
    for instance in instances {
        grouped
            .entry(instance.project.as_str())
            .or_default()
            .push(instance);
    }

    wln!(out, "    (instances");
    for (project, project_instances) in grouped {
        wln!(out, "      (project \"{}\"", escape(project));
        let mut sorted_instances = project_instances;
        sorted_instances.sort_by(|left, right| left.path.cmp(&right.path));
        for instance in sorted_instances {
            wln!(out, "        (path \"{}\"", escape(&instance.path));
            wln!(out, "          (page \"{}\")", escape(&instance.page));
            wln!(out, "        )");
        }
        wln!(out, "      )");
    }
    wln!(out, "    )");
}

fn write_property(out: &mut String, key: &str, value: &str, text: &TextProp, _sym_rot: f64) {
    // prop.rotation is stored in world-frame (same as KiCad file storage).
    // Write it back verbatim — the renderer composes sym+prop to get screen angle.
    let stored_rot = text.rotation;

    wln!(out, "    (property \"{}\" \"{}\"", key, escape(value));
    wln!(
        out,
        "      (at {} {} {})",
        fmt_f64(text.position.x),
        fmt_f64(text.position.y),
        fmt_f64(stored_rot)
    );
    wln!(out, "      (show_name no)");
    wln!(out, "      (do_not_autoplace no)");
    // KiCad 8: (hide yes) is a direct child of the property node, NOT inside
    // (effects ...).  Write it here so round-trips preserve visibility.
    if text.hidden {
        wln!(out, "      (hide yes)");
    }
    w!(
        out,
        "      (effects (font (size {} {}))",
        fmt_f64(text.font_size),
        fmt_f64(text.font_size)
    );
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
        fmt_f64(note.position.x),
        fmt_f64(note.position.y),
        fmt_f64(note.rotation)
    );
    wln!(out, "    (effects");
    wln!(out, "      (font");
    wln!(
        out,
        "        (size {} {})",
        fmt_f64(note.font_size),
        fmt_f64(note.font_size)
    );
    wln!(out, "      )");
    wln!(out, "    )");
    wln!(out, "    (uuid \"{}\")", note.uuid);
    wln!(out, "  )");
}

fn write_drawing(out: &mut String, d: &SchDrawing) {
    match d {
        SchDrawing::Line {
            uuid,
            start,
            end,
            width,
        } => {
            wln!(out, "  (polyline");
            wln!(
                out,
                "    (pts (xy {} {}) (xy {} {}))",
                fmt_f64(start.x),
                fmt_f64(start.y),
                fmt_f64(end.x),
                fmt_f64(end.y)
            );
            wln!(
                out,
                "    (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Polyline {
            uuid,
            points,
            width,
            fill,
        } => {
            wln!(out, "  (polyline");
            w!(out, "    (pts");
            for p in points {
                w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
            }
            wln!(out, ")");
            wln!(
                out,
                "    (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Circle {
            uuid,
            center,
            radius,
            width,
            fill,
        } => {
            wln!(out, "  (circle");
            wln!(
                out,
                "    (center {} {})",
                fmt_f64(center.x),
                fmt_f64(center.y)
            );
            wln!(out, "    (radius {})", fmt_f64(*radius));
            wln!(
                out,
                "    (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Arc {
            uuid,
            start,
            mid,
            end,
            width,
            fill,
        } => {
            wln!(out, "  (arc");
            wln!(out, "    (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "    (mid {} {})", fmt_f64(mid.x), fmt_f64(mid.y));
            wln!(out, "    (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(
                out,
                "    (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
        SchDrawing::Rect {
            uuid,
            start,
            end,
            width,
            fill,
        } => {
            wln!(out, "  (rectangle");
            wln!(out, "    (start {} {})", fmt_f64(start.x), fmt_f64(start.y));
            wln!(out, "    (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(
                out,
                "    (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "    (fill (type {}))", fill_type_str(*fill));
            wln!(out, "    (uuid \"{}\")", uuid);
            wln!(out, "  )");
        }
    }
}

fn write_child_sheet(out: &mut String, cs: &ChildSheet) {
    wln!(out, "  (sheet");
    wln!(
        out,
        "    (at {} {})",
        fmt_f64(cs.position.x),
        fmt_f64(cs.position.y)
    );
    wln!(
        out,
        "    (size {} {})",
        fmt_f64(cs.size.0),
        fmt_f64(cs.size.1)
    );
    if cs.fields_autoplaced {
        wln!(out, "    (fields_autoplaced)");
    }
    wln!(
        out,
        "    (stroke (width {}) (type default))",
        fmt_f64(cs.stroke_width)
    );
    wln!(out, "    (fill (type {}))", fill_type_str(cs.fill));
    wln!(out, "    (uuid \"{}\")", cs.uuid);
    wln!(out, "    (property \"Sheet name\" \"{}\"", escape(&cs.name));
    wln!(out, "      (id 0)");
    wln!(
        out,
        "      (at {} {} 0)",
        fmt_f64(cs.position.x),
        fmt_f64(cs.position.y - 1.0)
    );
    wln!(out, "      (show_name no)");
    wln!(out, "      (do_not_autoplace no)");
    wln!(
        out,
        "      (effects (font (size 1.27 1.27)) (justify left bottom))"
    );
    wln!(out, "    )");
    // Sheetfile property
    wln!(
        out,
        "    (property \"Sheet file\" \"{}\"",
        escape(&cs.filename)
    );
    wln!(out, "      (id 1)");
    wln!(
        out,
        "      (at {} {} 0)",
        fmt_f64(cs.position.x),
        fmt_f64(cs.position.y + cs.size.1 + 1.0)
    );
    wln!(out, "      (show_name no)");
    wln!(out, "      (do_not_autoplace no)");
    wln!(
        out,
        "      (effects (font (size 1.27 1.27)) (justify left top))"
    );
    wln!(out, "    )");
    // Sheet pins
    for pin in &cs.pins {
        wln!(out, "    (pin \"{}\" {}", escape(&pin.name), pin.direction);
        wln!(
            out,
            "      (at {} {} {})",
            fmt_f64(pin.position.x),
            fmt_f64(pin.position.y),
            fmt_f64(pin.rotation)
        );
        wln!(
            out,
            "      (effects (font (size 1.27 1.27)) (justify left))"
        );
        wln!(out, "      (uuid \"{}\")", pin.uuid);
        wln!(out, "    )");
    }
    write_sheet_instances(out, &cs.instances);
    wln!(out, "  )");
}

fn write_lib_symbol_property(out: &mut String, key: &str, value: &str, id: u32) {
    wln!(out, "      (property \"{}\" \"{}\"", key, escape(value));
    wln!(out, "        (id {})", id);
    wln!(out, "        (at 0 0 0)");
    wln!(out, "        (effects (font (size 1.27 1.27)))");
    wln!(out, "      )");
}

fn write_optional_lib_symbol_property(out: &mut String, key: &str, value: &str, id: u32) {
    if value.is_empty() {
        return;
    }

    write_lib_symbol_property(out, key, value, id);
}

// ---------------------------------------------------------------------------
// lib_symbol writer
// ---------------------------------------------------------------------------

fn write_lib_symbol(out: &mut String, _id: &str, lib: &LibSymbol) {
    wln!(out, "    (symbol \"{}\"", escape(&lib.id));
    wln!(
        out,
        "      (in_bom {})",
        if lib.in_bom { "yes" } else { "no" }
    );
    wln!(
        out,
        "      (on_board {})",
        if lib.on_board { "yes" } else { "no" }
    );
    wln!(
        out,
        "      (in_pos_files {})",
        if lib.in_pos_files { "yes" } else { "no" }
    );
    wln!(
        out,
        "      (duplicate_pin_numbers_are_jumpers {})",
        if lib.duplicate_pin_numbers_are_jumpers {
            "yes"
        } else {
            "no"
        }
    );
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
        wln!(
            out,
            "      (pin_names (offset {}))",
            fmt_f64(lib.pin_name_offset)
        );
    }

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
    write_lib_symbol_property(out, "Reference", reference, 0);
    write_lib_symbol_property(out, "Value", value, 1);
    write_lib_symbol_property(out, "Footprint", &lib.footprint, 2);
    write_lib_symbol_property(out, "Datasheet", &lib.datasheet, 3);
    write_optional_lib_symbol_property(out, "Description", &lib.description, 4);
    write_optional_lib_symbol_property(out, "ki_keywords", &lib.keywords, 5);
    write_optional_lib_symbol_property(out, "ki_fp_filters", &lib.fp_filters, 6);

    // Sub-symbol for graphics
    wln!(out, "      (symbol \"{}_0_1\"", base_name);
    for lg in &lib.graphics {
        write_lib_graphic(out, &lg.graphic);
    }
    wln!(out, "      )");

    // Sub-symbol for pins
    wln!(out, "      (symbol \"{}_1_1\"", base_name);
    for lp in &lib.pins {
        write_lib_pin(out, &lp.pin);
    }
    wln!(out, "      )");

    wln!(out, "    )");
}

fn write_lib_graphic(out: &mut String, g: &Graphic) {
    match g {
        Graphic::Polyline {
            points,
            width,
            fill,
        } => {
            wln!(out, "        (polyline");
            w!(out, "          (pts");
            for p in points {
                w!(out, " (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
            }
            wln!(out, ")");
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Rectangle {
            start,
            end,
            width,
            fill,
        } => {
            wln!(out, "        (rectangle");
            wln!(
                out,
                "          (start {} {})",
                fmt_f64(start.x),
                fmt_f64(start.y)
            );
            wln!(out, "          (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Circle {
            center,
            radius,
            width,
            fill,
        } => {
            wln!(out, "        (circle");
            wln!(
                out,
                "          (center {} {})",
                fmt_f64(center.x),
                fmt_f64(center.y)
            );
            wln!(out, "          (radius {})", fmt_f64(*radius));
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Arc {
            start,
            mid,
            end,
            width,
            fill,
        } => {
            wln!(out, "        (arc");
            wln!(
                out,
                "          (start {} {})",
                fmt_f64(start.x),
                fmt_f64(start.y)
            );
            wln!(out, "          (mid {} {})", fmt_f64(mid.x), fmt_f64(mid.y));
            wln!(out, "          (end {} {})", fmt_f64(end.x), fmt_f64(end.y));
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            wln!(out, "        )");
        }
        Graphic::Text {
            text,
            position,
            rotation,
            font_size,
            bold,
            italic,
            justify_h,
            justify_v,
        } => {
            wln!(out, "        (text \"{}\"", escape(text));
            wln!(
                out,
                "          (at {} {} {})",
                fmt_f64(position.x),
                fmt_f64(position.y),
                fmt_f64(*rotation)
            );
            w!(
                out,
                "          (effects (font (size {} {})",
                fmt_f64(*font_size),
                fmt_f64(*font_size)
            );
            if *bold {
                w!(out, " bold");
            }
            if *italic {
                w!(out, " italic");
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
        } => {
            wln!(out, "        (text_box \"{}\"", escape(text));
            wln!(
                out,
                "          (at {} {} {})",
                fmt_f64(position.x),
                fmt_f64(position.y),
                fmt_f64(*rotation)
            );
            wln!(
                out,
                "          (size {} {})",
                fmt_f64(size.x),
                fmt_f64(size.y)
            );
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
            w!(
                out,
                "          (effects (font (size {} {})",
                fmt_f64(*font_size),
                fmt_f64(*font_size)
            );
            if *bold {
                w!(out, " bold");
            }
            if *italic {
                w!(out, " italic");
            }
            wln!(out, "))");
            wln!(out, "        )");
        }
        Graphic::Bezier {
            points,
            width,
            fill,
        } => {
            wln!(out, "        (bezier");
            wln!(out, "          (pts");
            for p in points {
                wln!(out, "            (xy {} {})", fmt_f64(p.x), fmt_f64(p.y));
            }
            wln!(out, "          )");
            wln!(
                out,
                "          (stroke (width {}) (type default))",
                fmt_f64(*width)
            );
            wln!(out, "          (fill (type {}))", fill_type_str(*fill));
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
        fmt_f64(pin.position.x),
        fmt_f64(pin.position.y),
        fmt_f64(pin.rotation)
    );
    wln!(out, "          (length {})", fmt_f64(pin.length));
    if !pin.visible {
        wln!(out, "          (hide yes)");
    }
    w!(
        out,
        "          (name \"{}\" (effects (font (size 1.27 1.27))",
        escape(&pin.name)
    );
    if !pin.name_visible {
        w!(out, " (hide yes)");
    }
    wln!(out, "))");
    w!(
        out,
        "          (number \"{}\" (effects (font (size 1.27 1.27))",
        escape(&pin.number)
    );
    if !pin.number_visible {
        w!(out, " (hide yes)");
    }
    wln!(out, "))");
    wln!(out, "        )");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_not_connected_pins_as_no_connect() {
        assert_eq!(
            pin_electrical_str(PinElectricalType::NotConnected),
            "no_connect"
        );
    }

    #[test]
    fn writes_property_metadata_in_kicad_order() {
        let mut out = String::new();
        let text = TextProp {
            position: Point { x: 10.0, y: 20.0 },
            rotation: 0.0,
            font_size: 1.27,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
            hidden: false,
        };

        write_property(&mut out, "Reference", "R1", &text, 0.0);

        assert!(out.contains("(show_name no)"));
        assert!(out.contains("(do_not_autoplace no)"));
        assert!(out.contains("(effects (font (size 1.27 1.27)))"));
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
                font_size: 1.27,
                justify_h: HAlign::Center,
                justify_v: VAlign::Center,
                hidden: false,
            }),
            val_text: Some(TextProp {
                position: Point { x: 10.0, y: 12.0 },
                rotation: 0.0,
                font_size: 1.27,
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
        assert!(rendered.contains("(sheet_instances"));
        assert!(rendered.contains("(page \"7\")"));
        assert!(rendered.contains("(property \"Datasheet\" \"https://example.invalid/r1\""));
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

        assert!(out.contains("(in_pos_files yes)"));
        assert!(out.contains("(duplicate_pin_numbers_are_jumpers no)"));
        assert!(out.contains("(property \"Description\" \"Ethernet controller\""));
        assert!(out.contains("(property \"ki_keywords\" \"WIZnet Ethernet\""));
        assert!(out.contains("(property \"ki_fp_filters\" \"LQFP*\""));
    }

    #[test]
    fn writes_hidden_lib_pin_flag() {
        let mut out = String::new();
        let pin = Pin {
            pin_type: PinElectricalType::NotConnected,
            shape: PinShape::Line,
            position: Point { x: 20.32, y: 0.0 },
            rotation: 0.0,
            length: 0.0,
            name: "NC".to_string(),
            number: "7".to_string(),
            visible: false,
            name_visible: true,
            number_visible: true,
        };

        write_lib_pin(&mut out, &pin);
        assert!(out.contains("(hide yes)"));
    }
}
