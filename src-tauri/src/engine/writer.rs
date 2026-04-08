use crate::engine::parser::*;
use std::fmt::Write;

// String's Write impl is infallible — this macro avoids 211 `.unwrap()` calls
macro_rules! w {
    ($dst:expr, $($arg:tt)*) => { let _ = write!($dst, $($arg)*); };
}
macro_rules! wln {
    ($dst:expr, $($arg:tt)*) => { let _ = writeln!($dst, $($arg)*); };
}

/// Escape a string for KiCad S-expression output (backslashes and double quotes)
fn escape_kicad_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Serialize a SchematicSheet back to KiCad S-expression format (.kicad_sch)
pub fn write_schematic(sheet: &SchematicSheet) -> String {
    let mut out = String::with_capacity(64 * 1024);

    wln!(out, "(kicad_sch");
    wln!(out, "\t(version {})", sheet.version);
    wln!(out, "\t(generator \"signex\")");
    wln!(out, "\t(generator_version \"0.1\")");
    wln!(out, "\t(uuid \"{}\")", sheet.uuid);
    wln!(out, "\t(paper \"{}\")", sheet.paper_size);

    // Title block
    if !sheet.title_block.is_empty() {
        wln!(out, "\t(title_block");
        if let Some(title) = sheet.title_block.get("title") {
            wln!(out, "\t\t(title \"{}\")", escape_kicad_string(title));
        }
        if let Some(date) = sheet.title_block.get("date") {
            wln!(out, "\t\t(date \"{}\")", escape_kicad_string(date));
        }
        if let Some(rev) = sheet.title_block.get("rev") {
            wln!(out, "\t\t(rev \"{}\")", escape_kicad_string(rev));
        }
        if let Some(company) = sheet.title_block.get("company") {
            wln!(out, "\t\t(company \"{}\")", escape_kicad_string(company));
        }
        for i in 1..=9 {
            let key = format!("comment_{}", i);
            if let Some(comment) = sheet.title_block.get(&key) {
                wln!(out, "\t\t(comment {} \"{}\")", i, escape_kicad_string(comment));
            }
        }
        wln!(out, "\t)");
    }

    // lib_symbols
    if !sheet.lib_symbols.is_empty() {
        wln!(out, "\t(lib_symbols");
        for (id, lib) in &sheet.lib_symbols {
            write_lib_symbol(&mut out, id, lib);
        }
        wln!(out, "\t)");
    }

    // Junctions
    for j in &sheet.junctions {
        wln!(out, "\t(junction");
        wln!(out, "\t\t(at {} {})", j.position.x, j.position.y);
        wln!(out, "\t\t(diameter 0)");
        wln!(out, "\t\t(color 0 0 0 0)");
        wln!(out, "\t\t(uuid \"{}\")", j.uuid);
        wln!(out, "\t)");
    }

    // No connects
    for nc in &sheet.no_connects {
        wln!(out, "\t(no_connect");
        wln!(out, "\t\t(at {} {})", nc.position.x, nc.position.y);
        wln!(out, "\t\t(uuid \"{}\")", nc.uuid);
        wln!(out, "\t)");
    }

    // Buses
    for b in &sheet.buses {
        wln!(out, "\t(bus");
        wln!(out, "\t\t(pts");
        wln!(
            out,
            "\t\t\t(xy {} {}) (xy {} {})",
            b.start.x, b.start.y, b.end.x, b.end.y
        );
        wln!(out, "\t\t)");
        wln!(out, "\t\t(stroke (width 0) (type default) (color 0 0 0 0))");
        wln!(out, "\t\t(uuid \"{}\")", b.uuid);
        wln!(out, "\t)");
    }

    // Bus entries
    for be in &sheet.bus_entries {
        wln!(out, "\t(bus_entry");
        wln!(out, "\t\t(at {} {})", be.position.x, be.position.y);
        wln!(out, "\t\t(size {} {})", be.size.0, be.size.1);
        wln!(out, "\t\t(stroke (width 0) (type default) (color 0 0 0 0))");
        wln!(out, "\t\t(uuid \"{}\")", be.uuid);
        wln!(out, "\t)");
    }

    // Wires
    for w in &sheet.wires {
        wln!(out, "\t(wire");
        wln!(out, "\t\t(pts");
        wln!(
            out,
            "\t\t\t(xy {} {}) (xy {} {})",
            w.start.x, w.start.y, w.end.x, w.end.y
        );
        wln!(out, "\t\t)");
        wln!(out, "\t\t(stroke");
        wln!(out, "\t\t\t(width 0)");
        wln!(out, "\t\t\t(type default)");
        wln!(out, "\t\t)");
        wln!(out, "\t\t(uuid \"{}\")", w.uuid);
        wln!(out, "\t)");
    }

    // Labels
    for l in &sheet.labels {
        let keyword = match l.label_type {
            LabelType::Net => "label",
            LabelType::Global => "global_label",
            LabelType::Hierarchical => "hierarchical_label",
            LabelType::Power => "label",
        };
        wln!(out, "\t({} \"{}\"", keyword, escape_kicad_string(&l.text));
        if !l.shape.is_empty() {
            wln!(out, "\t\t(shape {})", escape_kicad_string(&l.shape));
        }
        wln!(
            out,
            "\t\t(at {} {} {})",
            l.position.x, l.position.y, l.rotation
        );
        wln!(out, "\t\t(effects");
        wln!(out, "\t\t\t(font");
        wln!(out, "\t\t\t\t(size {} {})", l.font_size, l.font_size);
        wln!(out, "\t\t\t)");
        if l.justify != "left"
            || matches!(l.label_type, LabelType::Global | LabelType::Hierarchical)
        {
            wln!(out, "\t\t\t(justify {})", escape_kicad_string(&l.justify));
        }
        wln!(out, "\t\t)");
        wln!(out, "\t\t(uuid \"{}\")", l.uuid);
        wln!(out, "\t)");
    }

    // Symbols (instances)
    for sym in &sheet.symbols {
        wln!(out, "\t(symbol");
        wln!(out, "\t\t(lib_id \"{}\")", escape_kicad_string(&sym.lib_id));
        wln!(
            out,
            "\t\t(at {} {} {})",
            sym.position.x, sym.position.y, sym.rotation
        );
        if sym.mirror_x {
            wln!(out, "\t\t(mirror x)");
        }
        if sym.mirror_y {
            wln!(out, "\t\t(mirror y)");
        }
        wln!(out, "\t\t(unit {})", sym.unit);
        if sym.locked {
            wln!(out, "\t\t(locked)");
        }
        wln!(
            out,
            "\t\t(exclude_from_sim {})",
            if sym.exclude_from_sim { "yes" } else { "no" }
        );
        wln!(
            out,
            "\t\t(in_bom {})",
            if sym.in_bom { "yes" } else { "no" }
        );
        wln!(
            out,
            "\t\t(on_board {})",
            if sym.on_board { "yes" } else { "no" }
        );
        wln!(out, "\t\t(dnp {})", if sym.dnp { "yes" } else { "no" });
        if sym.fields_autoplaced {
            wln!(out, "\t\t(fields_autoplaced yes)");
        }
        wln!(out, "\t\t(uuid \"{}\")", sym.uuid);

        // Reference property
        write_property(
            &mut out,
            "Reference",
            &sym.reference,
            &sym.ref_text,
            sym.rotation,
        );
        // Value property
        write_property(&mut out, "Value", &sym.value, &sym.val_text, sym.rotation);
        // Footprint property (hidden)
        wln!(
            out,
            "\t\t(property \"Footprint\" \"{}\"",
            escape_kicad_string(&sym.footprint)
        );
        wln!(out, "\t\t\t(at {} {} 0)", sym.position.x, sym.position.y);
        wln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (hide yes))");
        wln!(out, "\t\t)");

        // Custom fields
        for (key, value) in &sym.fields {
            wln!(
            out,
            "\t\t(property \"{}\" \"{}\"",
                escape_kicad_string(key),
                escape_kicad_string(value)
        );
            wln!(out, "\t\t\t(at {} {} 0)", sym.position.x, sym.position.y);
            wln!(out, "\t\t\t(effects (font (size 1.27 1.27)) (hide yes))");
            wln!(out, "\t\t)");
        }

        wln!(out, "\t)");
    }

    // Text notes
    // No ERC directives
    for ne in &sheet.no_erc_directives {
        wln!(out, "\t(no_erc");
        wln!(out, "\t\t(at {} {})", ne.position.x, ne.position.y);
        wln!(out, "\t\t(uuid \"{}\")", ne.uuid);
        wln!(out, "\t)");
    }

    for note in &sheet.text_notes {
        wln!(
            out,
            "\t(text \"{}\"",
            escape_kicad_string(&note.text.replace('\n', "\\n"))
        );
        wln!(out, "\t\t(exclude_from_sim no)");
        wln!(
            out,
            "\t\t(at {} {} {})",
            note.position.x, note.position.y, note.rotation
        );
        wln!(out, "\t\t(effects");
        wln!(out, "\t\t\t(font");
        wln!(out, "\t\t\t\t(size {} {})", note.font_size, note.font_size);
        wln!(out, "\t\t\t)");
        wln!(out, "\t\t)");
        wln!(out, "\t\t(uuid \"{}\")", note.uuid);
        wln!(out, "\t)");
    }

    // Rectangles
    for r in &sheet.rectangles {
        wln!(out, "\t(rectangle");
        wln!(out, "\t\t(start {} {})", r.start.x, r.start.y);
        wln!(out, "\t\t(end {} {})", r.end.x, r.end.y);
        wln!(out, "\t\t(stroke (width 0) (type {}))", escape_kicad_string(&r.stroke_type));
        wln!(out, "\t\t(fill (type none))");
        wln!(out, "\t\t(uuid \"{}\")", r.uuid);
        wln!(out, "\t)");
    }

    // Drawing objects
    for d in &sheet.drawings {
        match d {
            SchDrawing::Line {
                uuid,
                start,
                end,
                width,
            } => {
                wln!(out, "\t(polyline");
                wln!(
            out,
            "\t\t(pts (xy {} {}) (xy {} {}))",
                    start.x, start.y, end.x, end.y
        );
                wln!(out, "\t\t(stroke (width {}) (type default))", width);
                wln!(out, "\t\t(uuid \"{}\")", uuid);
                wln!(out, "\t)");
            }
            SchDrawing::Polyline {
                uuid,
                points,
                width,
                fill,
            } => {
                wln!(out, "\t(polyline");
                w!(out, "\t\t(pts");
                for p in points {
                    w!(out, " (xy {} {})", p.x, p.y);
                }
                wln!(out, ")");
                wln!(out, "\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t(uuid \"{}\")", uuid);
                wln!(out, "\t)");
            }
            SchDrawing::Circle {
                uuid,
                center,
                radius,
                width,
                fill,
            } => {
                wln!(out, "\t(circle");
                wln!(out, "\t\t(center {} {})", center.x, center.y);
                wln!(out, "\t\t(radius {})", radius);
                wln!(out, "\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t(uuid \"{}\")", uuid);
                wln!(out, "\t)");
            }
            SchDrawing::Arc {
                uuid,
                start,
                mid,
                end,
                width,
            } => {
                wln!(out, "\t(arc");
                wln!(out, "\t\t(start {} {})", start.x, start.y);
                wln!(out, "\t\t(mid {} {})", mid.x, mid.y);
                wln!(out, "\t\t(end {} {})", end.x, end.y);
                wln!(out, "\t\t(stroke (width {}) (type default))", width);
                wln!(out, "\t\t(uuid \"{}\")", uuid);
                wln!(out, "\t)");
            }
            SchDrawing::Rect {
                uuid,
                start,
                end,
                width,
                fill,
            } => {
                // Rects written as rectangles (distinct from section box rectangles)
                wln!(out, "\t(rectangle");
                wln!(out, "\t\t(start {} {})", start.x, start.y);
                wln!(out, "\t\t(end {} {})", end.x, end.y);
                wln!(out, "\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t(uuid \"{}\")", uuid);
                wln!(out, "\t)");
            }
        }
    }

    // Child sheets
    for sheet_ref in &sheet.child_sheets {
        wln!(out, "\t(sheet");
        wln!(
            out,
            "\t\t(at {} {})",
            sheet_ref.position.x, sheet_ref.position.y
        );
        wln!(out, "\t\t(size {} {})", sheet_ref.size.0, sheet_ref.size.1);
        wln!(out, "\t\t(uuid \"{}\")", sheet_ref.uuid);
        wln!(
            out,
            "\t\t(property \"Sheetname\" \"{}\"",
            escape_kicad_string(&sheet_ref.name)
        );
        wln!(
            out,
            "\t\t\t(at {} {} 0)",
            sheet_ref.position.x,
            sheet_ref.position.y - 1.0
        );
        wln!(
            out,
            "\t\t\t(effects (font (size 1.27 1.27)) (justify left bottom))"
        );
        wln!(out, "\t\t)");
        wln!(
            out,
            "\t\t(property \"Sheetfile\" \"{}\"",
            escape_kicad_string(&sheet_ref.filename)
        );
        wln!(
            out,
            "\t\t\t(at {} {} 0)",
            sheet_ref.position.x,
            sheet_ref.position.y + sheet_ref.size.1 + 1.0
        );
        wln!(
            out,
            "\t\t\t(effects (font (size 1.27 1.27)) (justify left top))"
        );
        wln!(out, "\t\t)");
        // Sheet pins
        for pin in &sheet_ref.pins {
            wln!(
            out,
            "\t\t(pin \"{}\" {}",
                escape_kicad_string(&pin.name),
                pin.direction
        );
            wln!(
            out,
            "\t\t\t(at {} {} {})",
                pin.position.x, pin.position.y, pin.rotation
        );
            wln!(
            out,
            "\t\t\t(effects (font (size 1.27 1.27)) (justify left))"
        );
            wln!(out, "\t\t\t(uuid \"{}\")", pin.uuid);
            wln!(out, "\t\t)");
        }
        wln!(out, "\t)");
    }

    wln!(out, ")");
    out
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

    wln!(
            out,
            "\t\t(property \"{}\" \"{}\"",
        key,
        escape_kicad_string(value)
        );
    wln!(
            out,
            "\t\t\t(at {} {} {})",
        text.position.x, text.position.y, stored_rot
        );
    w!(out, "\t\t\t(effects (font (size {} {}))", text.font_size, text.font_size);
    if text.hidden {
        w!(out, " (hide yes)");
    }
    if text.justify_h != "center" || text.justify_v != "center" {
        w!(out, " (justify");
        if text.justify_h != "center" {
            w!(out, " {}", text.justify_h);
        }
        if text.justify_v != "center" {
            w!(out, " {}", text.justify_v);
        }
        w!(out, ")");
    }
    wln!(out, ")");
    wln!(out, "\t\t)");
}

/// Serialize a standalone symbol library file (.kicad_sym)
pub fn write_symbol_library(symbols: &[(String, LibSymbol)]) -> String {
    let mut out = String::with_capacity(16 * 1024);
    wln!(out, "(kicad_symbol_lib");
    wln!(out, "\t(version 20231120)");
    wln!(out, "\t(generator \"signex\")");
    wln!(out, "\t(generator_version \"0.1\")");
    for (id, lib) in symbols {
        write_lib_symbol(&mut out, id, lib);
    }
    wln!(out, ")");
    out
}

fn write_lib_symbol(out: &mut String, _id: &str, lib: &LibSymbol) {
    // Write a minimal lib_symbol entry
    // For a full round-trip, we'd need to preserve the original S-expression
    // For now, write enough to make the file loadable
    wln!(out, "\t\t(symbol \"{}\"", escape_kicad_string(&lib.id));
    if !lib.show_pin_numbers {
        wln!(out, "\t\t\t(pin_numbers hide)");
    }
    if !lib.show_pin_names {
        wln!(
            out,
            "\t\t\t(pin_names (offset {}) hide)",
            lib.pin_name_offset
        );
    } else {
        wln!(out, "\t\t\t(pin_names (offset {}))", lib.pin_name_offset);
    }

    // Sub-symbols for graphics
    wln!(
            out,
            "\t\t\t(symbol \"{}_0_1\"",
        lib.id.split(':').next_back().unwrap_or(&lib.id)
        );
    for g in &lib.graphics {
        match g {
            Graphic::Polyline {
                points,
                width,
                fill,
            } => {
                wln!(out, "\t\t\t\t(polyline");
                w!(out, "\t\t\t\t\t(pts");
                for p in points {
                    w!(out, " (xy {} {})", p.x, p.y);
                }
                wln!(out, ")");
                wln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t\t\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t\t\t)");
            }
            Graphic::Rectangle {
                start,
                end,
                width,
                fill,
            } => {
                wln!(out, "\t\t\t\t(rectangle");
                wln!(out, "\t\t\t\t\t(start {} {})", start.x, start.y);
                wln!(out, "\t\t\t\t\t(end {} {})", end.x, end.y);
                wln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t\t\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t\t\t)");
            }
            Graphic::Circle {
                center,
                radius,
                width,
                fill,
            } => {
                wln!(out, "\t\t\t\t(circle");
                wln!(out, "\t\t\t\t\t(center {} {})", center.x, center.y);
                wln!(out, "\t\t\t\t\t(radius {})", radius);
                wln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width);
                wln!(
            out,
            "\t\t\t\t\t(fill (type {}))",
                    if *fill { "outline" } else { "none" }
        );
                wln!(out, "\t\t\t\t)");
            }
            Graphic::Arc {
                start,
                mid,
                end,
                width,
            } => {
                wln!(out, "\t\t\t\t(arc");
                wln!(out, "\t\t\t\t\t(start {} {})", start.x, start.y);
                wln!(out, "\t\t\t\t\t(mid {} {})", mid.x, mid.y);
                wln!(out, "\t\t\t\t\t(end {} {})", end.x, end.y);
                wln!(out, "\t\t\t\t\t(stroke (width {}) (type default))", width);
                wln!(out, "\t\t\t\t\t(fill (type none))");
                wln!(out, "\t\t\t\t)");
            }
        }
    }
    wln!(out, "\t\t\t)");

    // Sub-symbol for pins
    wln!(
            out,
            "\t\t\t(symbol \"{}_1_1\"",
        lib.id.split(':').next_back().unwrap_or(&lib.id)
        );
    for pin in &lib.pins {
        wln!(out, "\t\t\t\t(pin {} {}", escape_kicad_string(&pin.pin_type), escape_kicad_string(&pin.shape));
        wln!(
            out,
            "\t\t\t\t\t(at {} {} {})",
            pin.position.x, pin.position.y, pin.rotation
        );
        wln!(out, "\t\t\t\t\t(length {})", pin.length);
        wln!(
            out,
            "\t\t\t\t\t(name \"{}\" (effects (font (size 1.27 1.27))))",
            escape_kicad_string(&pin.name)
        );
        wln!(
            out,
            "\t\t\t\t\t(number \"{}\" (effects (font (size 1.27 1.27))))",
            escape_kicad_string(&pin.number)
        );
        wln!(out, "\t\t\t\t)");
    }
    wln!(out, "\t\t\t)");

    wln!(out, "\t\t)");
}
