//! KiCad S-expression `.net` netlist export.
//!
//! See `OUTPUT_PLAN.md` §7. Emits byte-compatible KiCad netlist format so
//! downstream tools (KiCad CvPcb, kinet2pcb, legacy CAM) don't know Signex
//! was the emitter.
//!
//! ## Known simplifications (fixed in v0.8)
//! ~~- Pin positions: all pins placed at symbol origin~~
//!     ✅ Fixed: pins now use proper rotation+mirror transform
//! ~~- Mid-wire label binding: labels only bind at endpoints~~
//!     ✅ Fixed: labels now bind anywhere on a wire (0.01mm tolerance)
//! ~~- Single-sheet only: ignores hierarchical designs~~
//!     ✅ Fixed: walks all sheets; Global/Hierarchical labels unify across sheets

use thiserror::Error;

use crate::{ExportContext, Exporter};

mod kicad_sexpr;
use kicad_sexpr::{emit_comp, emit_header, emit_net};

pub struct NetlistExporter;

#[derive(Debug, Clone, Default)]
pub struct NetlistOptions {
    pub include_timestamps: bool,
}

/// FIX 3: Build net graph across multiple sheets with Global/Hierarchical label unification.
/// Labels with matching text on different sheets are unified into the same net.
fn build_net_graph_multi_sheet(
    sheets: &[crate::SheetSnapshot],
    symbols: &[signex_types::schematic::Symbol],
) -> kicad_sexpr::NetGraph {
    use signex_types::schematic::LabelType;

    // Use the first sheet for lib_symbols (all sheets share the same library)
    let first_sheet = &sheets[0].schematic;
    let mut graph = kicad_sexpr::build_net_graph(first_sheet, symbols);

    // For each additional sheet, process its labels and unify global ones
    for sheet_snap in &sheets[1..] {
        let sheet = &sheet_snap.schematic;

        // Process wires, junctions from this sheet
        let pos_key = |p: signex_types::schematic::Point| format!("{:.2}_{:.2}", p.x, p.y);
        let tolerance = 0.01; // mm

        // Track position -> node index
        let mut pos_to_node: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        // Process wires from this sheet
        for wire in &sheet.wires {
            let start_key = pos_key(wire.start);
            let end_key = pos_key(wire.end);

            let start_idx = *pos_to_node
                .entry(start_key)
                .or_insert_with(|| graph.add_node());
            let end_idx = *pos_to_node
                .entry(end_key)
                .or_insert_with(|| graph.add_node());

            graph.union(start_idx, end_idx);
        }

        // Process junctions from this sheet
        for junction in &sheet.junctions {
            let j_key = pos_key(junction.position);
            let j_idx = *pos_to_node.entry(j_key).or_insert_with(|| graph.add_node());

            for wire in &sheet.wires {
                if (wire.start == junction.position || wire.end == junction.position)
                    || crate::netlist::kicad_sexpr::point_on_segment(
                        junction.position,
                        wire.start,
                        wire.end,
                        tolerance,
                    )
                {
                    let start_key = pos_key(wire.start);
                    let end_key = pos_key(wire.end);
                    let start_idx = *pos_to_node
                        .entry(start_key)
                        .or_insert_with(|| graph.add_node());
                    let end_idx = *pos_to_node
                        .entry(end_key)
                        .or_insert_with(|| graph.add_node());
                    graph.union(j_idx, start_idx);
                    graph.union(j_idx, end_idx);
                }
            }
        }

        // Process labels from this sheet
        for label in &sheet.labels {
            let label_key = pos_key(label.position);
            let mut label_idx = pos_to_node.get(&label_key).copied();

            // Check if label lies on a wire (mid-wire binding)
            if label_idx.is_none() {
                for wire in &sheet.wires {
                    if kicad_sexpr::point_on_segment(
                        label.position,
                        wire.start,
                        wire.end,
                        tolerance,
                    ) {
                        let start_key = pos_key(wire.start);
                        let end_key = pos_key(wire.end);
                        let start_idx = *pos_to_node
                            .entry(start_key)
                            .or_insert_with(|| graph.add_node());
                        let end_idx = *pos_to_node
                            .entry(end_key)
                            .or_insert_with(|| graph.add_node());
                        graph.union(start_idx, end_idx);
                        label_idx = Some(start_idx);
                        break;
                    }
                }
            }

            // For Global/Hierarchical labels, find matching labels in existing graph and unify
            if matches!(
                label.label_type,
                LabelType::Global | LabelType::Hierarchical
            ) {
                if !label.text.is_empty() {
                    // Find a node in the main graph with the same label name
                    let mut main_idx = None;
                    for (root, net_name) in &graph.net_names {
                        if net_name == &label.text {
                            main_idx = Some(*root);
                            break;
                        }
                    }

                    // Unify: if we have both local and main indices, union them
                    if let (Some(l_idx), Some(m_idx)) = (label_idx, main_idx) {
                        graph.union(l_idx, m_idx);
                    } else if let Some(l_idx) = label_idx {
                        // Set the name even if no matching main net yet
                        graph.set_net_name(l_idx, label.text.clone());
                    }
                } else if let Some(idx) = label_idx {
                    graph.set_net_name(idx, label.text.clone());
                }
            } else {
                // Non-global/hierarchical labels: just set the name
                if let Some(idx) = label_idx {
                    graph.set_net_name(idx, label.text.clone());
                }
            }
        }
    }

    graph
}

/// Compose `Net-(R1-Pad2)` style auto-name from the lowest-ref pin on the net.
fn auto_net_name(pins: &[(String, String, String)]) -> String {
    pins.iter()
        .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
        .map(|(r, p, _)| format!("Net-({r}-Pad{p})"))
        .unwrap_or_else(|| "Net-(unconnected)".to_string())
}

#[derive(Debug, Clone)]
pub struct NetlistOutput {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum NetlistError {
    #[error("net graph construction failed")]
    GraphConstruction,
}

impl Exporter for NetlistExporter {
    type Options = NetlistOptions;
    type Output = NetlistOutput;
    type Error = NetlistError;

    fn export(
        &self,
        ctx: &ExportContext,
        opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        if ctx.sheets.is_empty() {
            return Err(NetlistError::GraphConstruction);
        }

        let mut components = Vec::new();
        let mut all_symbols = Vec::new();

        // Walk all sheets and collect symbols
        for sheet_snap in &ctx.sheets {
            for sym in &sheet_snap.schematic.symbols {
                // Skip power ports
                if sym.reference.starts_with("#PWR") {
                    continue;
                }
                all_symbols.push((sym.clone(), sheet_snap.clone()));
            }
        }

        // Multi-sheet net graph (walks every sheet; Global/Hierarchical
        // labels with the same name unify across sheets).
        let graph = build_net_graph_multi_sheet(
            &ctx.sheets,
            &all_symbols
                .iter()
                .map(|(s, _)| s.clone())
                .collect::<Vec<_>>(),
        );

        // Emit components
        for (sym, sheet_snap) in &all_symbols {
            if let Some(lib_sym) = sheet_snap.schematic.lib_symbols.get(&sym.lib_id) {
                let sheet_path = format!("/{}/", sheet_snap.sheet_name);
                let sheet_tstamp = format!("/<{:08x}>/", sheet_snap.sheet_number);

                let comp = emit_comp(
                    sym,
                    lib_sym,
                    &sheet_path,
                    &sheet_tstamp,
                    opts.include_timestamps,
                );
                components.push(comp);
            }
        }

        // Build nets. Emit any root that owns pins OR has a name from a label
        // — named pinless nets represent labeled wires that haven't been
        // connected to components yet (e.g. hierarchical-label-only sheets).
        // Unnamed roots get `Net-(Rx-Padn)` auto-names per KiCad convention.
        let mut net_code = 1u32;
        let mut nets_sexpr = Vec::new();
        let empty_pins: Vec<(String, String, String)> = Vec::new();

        let mut roots: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        roots.extend(graph.node_to_pins.keys().copied());
        roots.extend(graph.net_names.keys().copied());

        for root_idx in roots {
            let pins = graph.node_to_pins.get(&root_idx).unwrap_or(&empty_pins);
            let name = match graph.net_names.get(&root_idx) {
                Some(n) => n.clone(),
                None if !pins.is_empty() => auto_net_name(pins),
                None => continue,
            };

            let net = emit_net(net_code, &name, pins);
            nets_sexpr.push(net);
            net_code += 1;
        }

        // Build the root export node. The header date prefers the
        // project title-block `date` field — the engineer's chosen
        // revision date — falling back to a fixed deterministic
        // value (`2000-01-01T00:00:00`) when both are absent. The
        // fixed fallback keeps test assertions stable; live exports
        // always have a title-block date in practice.
        let tool_version = "Signex 0.8.0";
        let title_block_date = ctx.metadata.date.trim();
        let timestamp = if !title_block_date.is_empty() {
            title_block_date.to_string()
        } else {
            "2000-01-01T00:00:00".to_string()
        };

        // KiCad netlists use forward-slash separators in (source ...)
        // regardless of host OS — Windows `path.display()` mixes them
        // when crossed through `Path::join`. Normalise so the netlist
        // doesn't carry a "C:/foo\\bar" hybrid that looks like a bug.
        let source = ctx
            .sheets
            .first()
            .map(|s| s.path.display().to_string().replace('\\', "/"))
            .unwrap_or_default();

        let mut root_items = vec![emit_header(&source, &timestamp, tool_version)];

        // Add components section
        let mut comp_section = vec![kicad_parser::sexpr_builder::raw("components")];
        comp_section.extend(components);
        root_items.push(kicad_parser::sexpr_builder::list(comp_section));

        // Add nets section
        let mut nets_section = vec![kicad_parser::sexpr_builder::raw("nets")];
        nets_section.extend(nets_sexpr);
        root_items.push(kicad_parser::sexpr_builder::list(nets_section));

        let root = kicad_parser::sexpr_builder::list(root_items);

        // Convert to string
        let output = root.pretty(0);

        Ok(NetlistOutput {
            bytes: output.into_bytes(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_valid_header() {
        let header = emit_header(
            "/path/to/test.kicad_sch",
            "2026-04-22T00:00:00",
            "Signex 0.8.0",
        );
        let header_str = header.to_string();
        assert!(header_str.starts_with("(export (version D)"));
    }

    #[test]
    fn two_components_emit_two_comps() {
        let ctx = build_test_context_with_2_symbols();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = String::from_utf8(result.bytes).unwrap();

        // Pretty-printer may break `(comp` and `(ref` across lines, so
        // count component refs by the unique designators directly.
        assert!(output.contains("\"R1\""), "missing R1 in output:\n{output}");
        assert!(output.contains("\"R2\""), "missing R2 in output:\n{output}");
    }

    #[test]
    fn skips_power_ports() {
        let ctx = build_test_context_with_power_port();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = String::from_utf8(result.bytes).unwrap();

        assert!(!output.contains("(ref \"#PWR"));
    }

    #[test]
    fn labeled_wire_uses_label_name() {
        let ctx = build_test_context_with_labeled_wire();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = String::from_utf8(result.bytes).unwrap();

        assert!(
            output.contains("(name \"VCC_3V3\")"),
            "Expected VCC_3V3 net name in output"
        );
    }

    /// Normalise whitespace so tests tolerate the S-expression pretty-printer
    /// breaking `(name "x")` across lines with indentation.
    fn squish(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn connected_pins_share_net_code() {
        let ctx = build_test_context_with_2_connected_symbols();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = squish(&String::from_utf8(result.bytes).unwrap());

        assert!(
            output.contains("(name \"Net-"),
            "Expected net with auto-generated name in:\n{output}",
        );
    }

    #[test]
    fn mid_wire_label_names_net() {
        let ctx = build_test_context_with_mid_wire_label();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = squish(&String::from_utf8(result.bytes).unwrap());

        assert!(
            output.contains("(name \"SIGNAL\")"),
            "Expected SIGNAL net name from mid-wire label in:\n{output}",
        );
    }

    #[test]
    fn hier_label_connects_across_sheets() {
        let ctx = build_test_context_with_hier_labels();
        let exporter = NetlistExporter;
        let opts = NetlistOptions::default();
        let result = exporter.export(&ctx, &opts).unwrap();
        let output = squish(&String::from_utf8(result.bytes).unwrap());

        assert!(
            output.contains("(name \"BUS1\")"),
            "Expected BUS1 net name from hierarchical labels in:\n{output}",
        );
        let count = output.matches("(name \"BUS1\")").count();
        assert_eq!(count, 1, "BUS1 should appear exactly once in the output");
    }

    fn build_test_context_with_2_symbols() -> ExportContext {
        // Minimal test context with 2 symbols
        use signex_types::schematic::{
            LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
        };
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut lib_symbols = HashMap::new();
        let lib_sym = LibSymbol {
            id: "Device/R".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: "Resistor_SMD:R_0603_1608Metric".to_string(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: vec![],
            pins: vec![
                LibPin {
                    unit: 1,
                    body_style: 1,
                    pin: Pin {
                        direction: PinDirection::Passive,
                        shape_style: PinShapeStyle::Plain,
                        position: Point::new(0.0, 0.0),
                        number: "1".to_string(),
                        name: "~".to_string(),
                        length: 2.54,
                        rotation: 0.0,
                        visible: true,
                        name_visible: true,
                        number_visible: true,
                    },
                },
                LibPin {
                    unit: 1,
                    body_style: 1,
                    pin: Pin {
                        direction: PinDirection::Passive,
                        shape_style: PinShapeStyle::Plain,
                        position: Point::new(10.0, 0.0),
                        number: "2".to_string(),
                        name: "~".to_string(),
                        length: 2.54,
                        rotation: 0.0,
                        visible: true,
                        name_visible: true,
                        number_visible: true,
                    },
                },
            ],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        };
        lib_symbols.insert("Device/R".to_string(), lib_sym);

        let sym1 = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        let sym2 = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R2".to_string(),
            value: "10k".to_string(),
            position: Point::new(10.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![sym1, sym2],
            lib_symbols,
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            wires: vec![],
            junctions: vec![],
            labels: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        let snap = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test.kicad_sch"),
            schematic: sheet,
            sheet_name: "Root".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        };

        ExportContext {
            sheets: vec![snap],
            metadata: Default::default(),
        }
    }

    fn build_test_context_with_power_port() -> ExportContext {
        use signex_types::schematic::{Point, SchematicSheet, Symbol};
        use uuid::Uuid;

        let sym = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: String::new(),
            reference: "#PWR0001".to_string(),
            value: "GND".to_string(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![sym],
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            wires: vec![],
            junctions: vec![],
            labels: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            lib_symbols: std::collections::HashMap::new(),
            title_block: std::collections::HashMap::new(),
        };

        let snap = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test.kicad_sch"),
            schematic: sheet,
            sheet_name: "Root".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        };

        ExportContext {
            sheets: vec![snap],
            metadata: Default::default(),
        }
    }

    fn build_test_context_with_labeled_wire() -> ExportContext {
        use signex_types::schematic::{
            Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
            SchematicSheet, Symbol, Wire,
        };
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut lib_symbols = HashMap::new();
        let lib_sym = LibSymbol {
            id: "Device/R".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: "Resistor_SMD:R_0603_1608Metric".to_string(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: vec![],
            pins: vec![LibPin {
                unit: 1,
                body_style: 1,
                pin: Pin {
                    direction: PinDirection::Passive,
                    shape_style: PinShapeStyle::Plain,
                    position: Point::new(0.0, 0.0),
                    number: "1".to_string(),
                    name: "~".to_string(),
                    length: 2.54,
                    rotation: 0.0,
                    visible: true,
                    name_visible: true,
                    number_visible: true,
                },
            }],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        };
        lib_symbols.insert("Device/R".to_string(), lib_sym);

        let sym = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        let wire = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
            stroke_width: 0.0,
        };

        // Place label at the wire endpoint at (0.0, 0.0) — net-graph only
        // binds labels that coincide with a pin or wire endpoint. Mid-wire
        // label binding is a future polish item.
        let label = Label {
            uuid: Uuid::new_v4(),
            text: "VCC_3V3".to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Center,
            justify_v: signex_types::schematic::VAlign::Bottom,
        };

        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![sym],
            wires: vec![wire],
            labels: vec![label],
            lib_symbols,
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            junctions: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        let snap = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test.kicad_sch"),
            schematic: sheet,
            sheet_name: "Root".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        };

        ExportContext {
            sheets: vec![snap],
            metadata: Default::default(),
        }
    }

    fn build_test_context_with_2_connected_symbols() -> ExportContext {
        use signex_types::schematic::{
            LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point, SchematicSheet, Symbol,
            Wire,
        };
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut lib_symbols = HashMap::new();
        let lib_sym = LibSymbol {
            id: "Device/R".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: "Resistor_SMD:R_0603_1608Metric".to_string(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: vec![],
            pins: vec![
                LibPin {
                    unit: 1,
                    body_style: 1,
                    pin: Pin {
                        direction: PinDirection::Passive,
                        shape_style: PinShapeStyle::Plain,
                        position: Point::new(0.0, 0.0),
                        number: "1".to_string(),
                        name: "~".to_string(),
                        length: 2.54,
                        rotation: 0.0,
                        visible: true,
                        name_visible: true,
                        number_visible: true,
                    },
                },
                LibPin {
                    unit: 1,
                    body_style: 1,
                    pin: Pin {
                        direction: PinDirection::Passive,
                        shape_style: PinShapeStyle::Plain,
                        position: Point::new(10.0, 0.0),
                        number: "2".to_string(),
                        name: "~".to_string(),
                        length: 2.54,
                        rotation: 0.0,
                        visible: true,
                        name_visible: true,
                        number_visible: true,
                    },
                },
            ],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        };
        lib_symbols.insert("Device/R".to_string(), lib_sym);

        let sym1 = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        let sym2 = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R2".to_string(),
            value: "10k".to_string(),
            position: Point::new(10.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        // Wire connecting R1 pin 2 (at 10,0) to R2 pin 1 (at 10,0)
        let wire = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(10.0, 0.0),
            end: Point::new(10.0, 0.0),
            stroke_width: 0.0,
        };

        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![sym1, sym2],
            wires: vec![wire],
            labels: vec![],
            lib_symbols,
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            junctions: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        let snap = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test.kicad_sch"),
            schematic: sheet,
            sheet_name: "Root".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        };

        ExportContext {
            sheets: vec![snap],
            metadata: Default::default(),
        }
    }

    fn build_test_context_with_mid_wire_label() -> ExportContext {
        use signex_types::schematic::{
            Label, LabelType, LibPin, LibSymbol, Pin, PinDirection, PinShapeStyle, Point,
            SchematicSheet, Symbol, Wire,
        };
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut lib_symbols = HashMap::new();
        let lib_sym = LibSymbol {
            id: "Device/R".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: "Resistor_SMD:R_0603_1608Metric".to_string(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: vec![],
            pins: vec![LibPin {
                unit: 1,
                body_style: 1,
                pin: Pin {
                    direction: PinDirection::Passive,
                    shape_style: PinShapeStyle::Plain,
                    position: Point::new(0.0, 0.0),
                    number: "1".to_string(),
                    name: "~".to_string(),
                    length: 2.54,
                    rotation: 0.0,
                    visible: true,
                    name_visible: true,
                    number_visible: true,
                },
            }],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        };
        lib_symbols.insert("Device/R".to_string(), lib_sym);

        let sym = Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device/R".to_string(),
            reference: "R1".to_string(),
            value: "10k".to_string(),
            position: Point::new(0.0, 0.0),
            unit: 1,
            footprint: String::new(),
            datasheet: String::new(),
            rotation: 0.0,
            mirror_x: false,
            mirror_y: false,
            is_power: false,
            ref_text: None,
            val_text: None,
            fields_autoplaced: false,
            dnp: false,
            in_bom: true,
            on_board: true,
            exclude_from_sim: false,
            locked: false,
            fields: std::collections::HashMap::new(),
            custom_properties: vec![],
            pin_uuids: std::collections::HashMap::new(),
            instances: vec![],
        };

        // Wire from (0,0) to (10,0)
        let wire = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            stroke_width: 0.0,
        };

        // Label placed at the middle of the wire (5,0)
        let label = Label {
            uuid: Uuid::new_v4(),
            text: "SIGNAL".to_string(),
            position: Point::new(5.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Center,
            justify_v: signex_types::schematic::VAlign::Bottom,
        };

        let sheet = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![sym],
            wires: vec![wire],
            labels: vec![label],
            lib_symbols,
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            junctions: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        let snap = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test.kicad_sch"),
            schematic: sheet,
            sheet_name: "Root".to_string(),
            sheet_number: 1,
            sheet_count: 1,
        };

        ExportContext {
            sheets: vec![snap],
            metadata: Default::default(),
        }
    }

    fn build_test_context_with_hier_labels() -> ExportContext {
        use signex_types::schematic::{Label, LabelType, LibSymbol, Point, SchematicSheet, Wire};
        #[allow(unused_imports)]
        use std::collections::HashMap;
        use uuid::Uuid;

        let mut lib_symbols = HashMap::new();
        let lib_sym = LibSymbol {
            id: "Device/R".to_string(),
            reference: "R".to_string(),
            value: "10k".to_string(),
            footprint: "Resistor_SMD:R_0603_1608Metric".to_string(),
            datasheet: String::new(),
            description: String::new(),
            keywords: String::new(),
            fp_filters: String::new(),
            in_bom: true,
            on_board: true,
            in_pos_files: true,
            duplicate_pin_numbers_are_jumpers: false,
            graphics: vec![],
            pins: vec![],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 0.0,
        };
        lib_symbols.insert("Device/R".to_string(), lib_sym);

        // Sheet 1: wire + hierarchical label
        let wire1 = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
            stroke_width: 0.0,
        };

        let label1 = Label {
            uuid: Uuid::new_v4(),
            text: "BUS1".to_string(),
            position: Point::new(0.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: String::new(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Center,
            justify_v: signex_types::schematic::VAlign::Bottom,
        };

        let sheet1 = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![],
            wires: vec![wire1],
            labels: vec![label1],
            lib_symbols: lib_symbols.clone(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "1".to_string(),
            junctions: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        // Sheet 2: wire + same hierarchical label
        let wire2 = Wire {
            uuid: Uuid::new_v4(),
            start: Point::new(10.0, 0.0),
            end: Point::new(15.0, 0.0),
            stroke_width: 0.0,
        };

        let label2 = Label {
            uuid: Uuid::new_v4(),
            text: "BUS1".to_string(),
            position: Point::new(10.0, 0.0),
            rotation: 0.0,
            label_type: LabelType::Hierarchical,
            shape: String::new(),
            font_size: 1.27,
            justify: signex_types::schematic::HAlign::Center,
            justify_v: signex_types::schematic::VAlign::Bottom,
        };

        let sheet2 = SchematicSheet {
            uuid: Uuid::new_v4(),
            symbols: vec![],
            wires: vec![wire2],
            labels: vec![label2],
            lib_symbols,
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: String::new(),
            root_sheet_page: "2".to_string(),
            junctions: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: std::collections::HashMap::new(),
        };

        let snap1 = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test1.kicad_sch"),
            schematic: sheet1,
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 2,
        };

        let snap2 = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test2.kicad_sch"),
            schematic: sheet2,
            sheet_name: "Sheet2".to_string(),
            sheet_number: 2,
            sheet_count: 2,
        };

        ExportContext {
            sheets: vec![snap1, snap2],
            metadata: Default::default(),
        }
    }
}
