use crate::engine::parser::*;
use std::fmt::Write;

/// Serialize a SchematicSheet back to KiCad S-expression format (.kicad_sch)
pub fn write_schematic(sheet: &SchematicSheet) -> String {
    let mut out = String::with_capacity(64 * 1024);

    writeln!(out, "(kicad_sch").unwrap();
    writeln!(out, "\t(version {})", sheet.version).unwrap();
    writeln!(out, "\t(generator \"signex\")").unwrap();
    writeln!(out, "\t(generator_version \"0.1\")").unwrap();
    writeln!(out, "\t(uuid \"{}\")", sheet.uuid).unwrap();
    writeln!(out, "\t(paper \"{}\")", sheet.paper_size).unwrap();

    // lib_symbols
    if !sheet.lib_symbols.is_empty() {
        writeln!(out, "\t(lib_symbols").unwrap();
        for (id, lib) in &sheet.lib_symbols {
            write_lib_symbol(&mut out, id, lib);
        }
        writeln!(out, "\t)").unwrap();
    }

    // Junctions
    for j in &sheet.junctions {
        writeln!(out, "\t(junction").unwrap();
        writeln!(out, "\t\t(at {} {})", j.position.x, j.position.y).unwrap();
        writeln!(out, "\t\t(diameter 0)").unwrap();
        writeln!(out, "\t\t(color 0 0 0 0)").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", j.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // No connects
    for nc in &sheet.no_connects {
        writeln!(out, "\t(no_connect").unwrap();
        writeln!(out, "\t\t(at {} {})", nc.position.x, nc.position.y).unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", nc.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Buses
    for b in &sheet.buses {
        writeln!(out, "\t(bus").unwrap();
        writeln!(out, "\t\t(pts").unwrap();
        writeln!(out, "\t\t\t(xy {} {}) (xy {} {})", b.start.x, b.start.y, b.end.x, b.end.y).unwrap();
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(stroke (width 0) (type default) (color 0 0 0 0))").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", b.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Bus entries
    for be in &sheet.bus_entries {
        writeln!(out, "\t(bus_entry").unwrap();
        writeln!(out, "\t\t(at {} {})", be.position.x, be.position.y).unwrap();
        writeln!(out, "\t\t(size {} {})", be.size.0, be.size.1).unwrap();
        writeln!(out, "\t\t(stroke (width 0) (type default) (color 0 0 0 0))").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", be.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Wires
    for w in &sheet.wires {
        writeln!(out, "\t(wire").unwrap();
        writeln!(out, "\t\t(pts").unwrap();
        writeln!(out, "\t\t\t(xy {} {}) (xy {} {})", w.start.x, w.start.y, w.end.x, w.end.y).unwrap();
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(stroke").unwrap();
        writeln!(out, "\t\t\t(width 0)").unwrap();
        writeln!(out, "\t\t\t(type default)").unwrap();
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", w.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Labels
    for l in &sheet.labels {
        let keyword = match l.label_type {
            LabelType::Net => "label",
            LabelType::Global => "global_label",
            LabelType::Hierarchical => "hierarchical_label",
            LabelType::Power => "label",
        };
        writeln!(out, "\t({} \"{}\"", keyword, l.text).unwrap();
        if !l.shape.is_empty() {
            writeln!(out, "\t\t(shape {})", l.shape).unwrap();
        }
        writeln!(out, "\t\t(at {} {} {})", l.position.x, l.position.y, l.rotation).unwrap();
        writeln!(out, "\t\t(effects").unwrap();
        writeln!(out, "\t\t\t(font").unwrap();
        writeln!(out, "\t\t\t\t(size {} {})", l.font_size, l.font_size).unwrap();
        writeln!(out, "\t\t\t)").unwrap();
        if l.justify != "left" || matches!(l.label_type, LabelType::Global | LabelType::Hierarchical) {
            writeln!(out, "\t\t\t(justify {})", l.justify).unwrap();
        }
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", l.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Symbols (instances)
    for sym in &sheet.symbols {
        writeln!(out, "\t(symbol").unwrap();
        writeln!(out, "\t\t(lib_id \"{}\")", sym.lib_id).unwrap();
        writeln!(out, "\t\t(at {} {} {})", sym.position.x, sym.position.y, sym.rotation).unwrap();
        if sym.mirror_x { writeln!(out, "\t\t(mirror x)").unwrap(); }
        if sym.mirror_y { writeln!(out, "\t\t(mirror y)").unwrap(); }
        writeln!(out, "\t\t(unit {})", sym.unit).unwrap();
        if sym.locked { writeln!(out, "\t\t(locked)").unwrap(); }
        writeln!(out, "\t\t(exclude_from_sim {})", if sym.exclude_from_sim { "yes" } else { "no" }).unwrap();
        writeln!(out, "\t\t(in_bom {})", if sym.in_bom { "yes" } else { "no" }).unwrap();
        writeln!(out, "\t\t(on_board {})", if sym.on_board { "yes" } else { "no" }).unwrap();
        writeln!(out, "\t\t(dnp {})", if sym.dnp { "yes" } else { "no" }).unwrap();
        if sym.fields_autoplaced {
            writeln!(out, "\t\t(fields_autoplaced yes)").unwrap();
        }
        writeln!(out, "\t\t(uuid \"{}\")", sym.uuid).unwrap();

        // Reference property
        write_property(&mut out, "Reference", &sym.reference, &sym.ref_text, sym.rotation);
        // Value property
        write_property(&mut out, "Value", &sym.value, &sym.val_text, sym.rotation);
        // Footprint property (hidden)
        writeln!(out, "\t\t(property \"Footprint\" \"{}\"", sym.footprint).unwrap();
        writeln!(out, "\t\t\t(at {} {} 0)", sym.position.x, sym.position.y).unwrap();
        writeln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (hide yes))").unwrap();
        writeln!(out, "\t\t)").unwrap();

        writeln!(out, "\t)").unwrap();
    }

    // Text notes
    for note in &sheet.text_notes {
        writeln!(out, "\t(text \"{}\"", note.text.replace('\n', "\\n")).unwrap();
        writeln!(out, "\t\t(exclude_from_sim no)").unwrap();
        writeln!(out, "\t\t(at {} {} {})", note.position.x, note.position.y, note.rotation).unwrap();
        writeln!(out, "\t\t(effects").unwrap();
        writeln!(out, "\t\t\t(font").unwrap();
        writeln!(out, "\t\t\t\t(size {} {})", note.font_size, note.font_size).unwrap();
        writeln!(out, "\t\t\t)").unwrap();
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", note.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Rectangles
    for r in &sheet.rectangles {
        writeln!(out, "\t(rectangle").unwrap();
        writeln!(out, "\t\t(start {} {})", r.start.x, r.start.y).unwrap();
        writeln!(out, "\t\t(end {} {})", r.end.x, r.end.y).unwrap();
        writeln!(out, "\t\t(stroke (width 0) (type {}))", r.stroke_type).unwrap();
        writeln!(out, "\t\t(fill (type none))").unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", r.uuid).unwrap();
        writeln!(out, "\t)").unwrap();
    }

    // Drawing objects
    for d in &sheet.drawings {
        match d {
            SchDrawing::Line { uuid, start, end, width } => {
                writeln!(out, "\t(polyline").unwrap();
                writeln!(out, "\t\t(pts (xy {} {}) (xy {} {}))", start.x, start.y, end.x, end.y).unwrap();
                writeln!(out, "\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t(uuid \"{}\")", uuid).unwrap();
                writeln!(out, "\t)").unwrap();
            }
            SchDrawing::Polyline { uuid, points, width, fill } => {
                writeln!(out, "\t(polyline").unwrap();
                write!(out, "\t\t(pts").unwrap();
                for p in points { write!(out, " (xy {} {})", p.x, p.y).unwrap(); }
                writeln!(out, ")").unwrap();
                writeln!(out, "\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t(uuid \"{}\")", uuid).unwrap();
                writeln!(out, "\t)").unwrap();
            }
            SchDrawing::Circle { uuid, center, radius, width, fill } => {
                writeln!(out, "\t(circle").unwrap();
                writeln!(out, "\t\t(center {} {})", center.x, center.y).unwrap();
                writeln!(out, "\t\t(radius {})", radius).unwrap();
                writeln!(out, "\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t(uuid \"{}\")", uuid).unwrap();
                writeln!(out, "\t)").unwrap();
            }
            SchDrawing::Arc { uuid, start, mid, end, width } => {
                writeln!(out, "\t(arc").unwrap();
                writeln!(out, "\t\t(start {} {})", start.x, start.y).unwrap();
                writeln!(out, "\t\t(mid {} {})", mid.x, mid.y).unwrap();
                writeln!(out, "\t\t(end {} {})", end.x, end.y).unwrap();
                writeln!(out, "\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t(uuid \"{}\")", uuid).unwrap();
                writeln!(out, "\t)").unwrap();
            }
            SchDrawing::Rect { uuid, start, end, width, fill } => {
                // Rects written as rectangles (distinct from section box rectangles)
                writeln!(out, "\t(rectangle").unwrap();
                writeln!(out, "\t\t(start {} {})", start.x, start.y).unwrap();
                writeln!(out, "\t\t(end {} {})", end.x, end.y).unwrap();
                writeln!(out, "\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t(uuid \"{}\")", uuid).unwrap();
                writeln!(out, "\t)").unwrap();
            }
        }
    }

    // Child sheets
    for sheet_ref in &sheet.child_sheets {
        writeln!(out, "\t(sheet").unwrap();
        writeln!(out, "\t\t(at {} {})", sheet_ref.position.x, sheet_ref.position.y).unwrap();
        writeln!(out, "\t\t(size {} {})", sheet_ref.size.0, sheet_ref.size.1).unwrap();
        writeln!(out, "\t\t(uuid \"{}\")", sheet_ref.uuid).unwrap();
        writeln!(out, "\t\t(property \"Sheetname\" \"{}\"", sheet_ref.name).unwrap();
        writeln!(out, "\t\t\t(at {} {} 0)", sheet_ref.position.x, sheet_ref.position.y - 1.0).unwrap();
        writeln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (justify left bottom))").unwrap();
        writeln!(out, "\t\t)").unwrap();
        writeln!(out, "\t\t(property \"Sheetfile\" \"{}\"", sheet_ref.filename).unwrap();
        writeln!(out, "\t\t\t(at {} {} 0)", sheet_ref.position.x, sheet_ref.position.y + sheet_ref.size.1 + 1.0).unwrap();
        writeln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (justify left top))").unwrap();
        writeln!(out, "\t\t)").unwrap();
        // Sheet pins
        for pin in &sheet_ref.pins {
            writeln!(out, "\t\t(pin \"{}\" {}", pin.name, pin.direction).unwrap();
            writeln!(out, "\t\t\t(at {} {} {})", pin.position.x, pin.position.y, pin.rotation).unwrap();
            writeln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (justify left))").unwrap();
            writeln!(out, "\t\t\t(uuid \"{}\")", pin.uuid).unwrap();
            writeln!(out, "\t\t)").unwrap();
        }
        writeln!(out, "\t)").unwrap();
    }

    writeln!(out, ")").unwrap();
    out
}

fn write_property(out: &mut String, key: &str, value: &str, text: &TextProp, sym_rot: f64) {
    // Reconstruct stored rotation (reverse the toggle applied during parsing)
    let sym_90_270 = (sym_rot - 90.0).abs() < 0.1 || (sym_rot - 270.0).abs() < 0.1;
    let stored_rot = if sym_90_270 {
        if text.rotation.abs() < 0.1 { 90.0 } else { 0.0 }
    } else {
        text.rotation
    };

    writeln!(out, "\t\t(property \"{}\" \"{}\"", key, value).unwrap();
    writeln!(out, "\t\t\t(at {} {} {})", text.position.x, text.position.y, stored_rot).unwrap();
    write!(out, "\t\t\t(effects (font (size {} {}))", text.font_size, text.font_size).unwrap();
    if text.hidden {
        write!(out, " (hide yes)").unwrap();
    }
    if text.justify_h != "center" || text.justify_v != "center" {
        write!(out, " (justify").unwrap();
        if text.justify_h != "center" { write!(out, " {}", text.justify_h).unwrap(); }
        if text.justify_v != "center" { write!(out, " {}", text.justify_v).unwrap(); }
        write!(out, ")").unwrap();
    }
    writeln!(out, ")").unwrap();
    writeln!(out, "\t\t)").unwrap();
}

fn write_lib_symbol(out: &mut String, _id: &str, lib: &LibSymbol) {
    // Write a minimal lib_symbol entry
    // For a full round-trip, we'd need to preserve the original S-expression
    // For now, write enough to make the file loadable
    writeln!(out, "\t\t(symbol \"{}\"", lib.id).unwrap();
    if !lib.show_pin_numbers {
        writeln!(out, "\t\t\t(pin_numbers hide)").unwrap();
    }
    if !lib.show_pin_names {
        writeln!(out, "\t\t\t(pin_names (offset {}) hide)", lib.pin_name_offset).unwrap();
    } else {
        writeln!(out, "\t\t\t(pin_names (offset {}))", lib.pin_name_offset).unwrap();
    }

    // Sub-symbols for graphics
    writeln!(out, "\t\t\t(symbol \"{}_0_1\"", lib.id.split(':').last().unwrap_or(&lib.id)).unwrap();
    for g in &lib.graphics {
        match g {
            Graphic::Polyline { points, width, fill } => {
                writeln!(out, "\t\t\t\t(polyline").unwrap();
                write!(out, "\t\t\t\t\t(pts").unwrap();
                for p in points {
                    write!(out, " (xy {} {})", p.x, p.y).unwrap();
                }
                writeln!(out, ")").unwrap();
                writeln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t\t\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t\t\t)").unwrap();
            }
            Graphic::Rectangle { start, end, width, fill } => {
                writeln!(out, "\t\t\t\t(rectangle").unwrap();
                writeln!(out, "\t\t\t\t\t(start {} {})", start.x, start.y).unwrap();
                writeln!(out, "\t\t\t\t\t(end {} {})", end.x, end.y).unwrap();
                writeln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t\t\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t\t\t)").unwrap();
            }
            Graphic::Circle { center, radius, width, fill } => {
                writeln!(out, "\t\t\t\t(circle").unwrap();
                writeln!(out, "\t\t\t\t\t(center {} {})", center.x, center.y).unwrap();
                writeln!(out, "\t\t\t\t\t(radius {})", radius).unwrap();
                writeln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t\t\t\t(fill (type {}))", if *fill { "outline" } else { "none" }).unwrap();
                writeln!(out, "\t\t\t\t)").unwrap();
            }
            Graphic::Arc { start, mid, end, width } => {
                writeln!(out, "\t\t\t\t(arc").unwrap();
                writeln!(out, "\t\t\t\t\t(start {} {})", start.x, start.y).unwrap();
                writeln!(out, "\t\t\t\t\t(mid {} {})", mid.x, mid.y).unwrap();
                writeln!(out, "\t\t\t\t\t(end {} {})", end.x, end.y).unwrap();
                writeln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width).unwrap();
                writeln!(out, "\t\t\t\t\t(fill (type none))").unwrap();
                writeln!(out, "\t\t\t\t)").unwrap();
            }
        }
    }
    writeln!(out, "\t\t\t)").unwrap();

    // Sub-symbol for pins
    writeln!(out, "\t\t\t(symbol \"{}_1_1\"", lib.id.split(':').last().unwrap_or(&lib.id)).unwrap();
    for pin in &lib.pins {
        writeln!(out, "\t\t\t\t(pin {} {}", pin.pin_type, pin.shape).unwrap();
        writeln!(out, "\t\t\t\t\t(at {} {} {})", pin.position.x, pin.position.y, pin.rotation).unwrap();
        writeln!(out, "\t\t\t\t\t(length {})", pin.length).unwrap();
        writeln!(out, "\t\t\t\t\t(name \"{}\" (effects (font (size 1.27 1.27))))", pin.name).unwrap();
        writeln!(out, "\t\t\t\t\t(number \"{}\" (effects (font (size 1.27 1.27))))", pin.number).unwrap();
        writeln!(out, "\t\t\t\t)").unwrap();
    }
    writeln!(out, "\t\t\t)").unwrap();

    writeln!(out, "\t\t)").unwrap();
}
