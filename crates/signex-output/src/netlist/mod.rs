//! KiCad S-expression `.net` netlist export.
//!
//! See `OUTPUT_PLAN.md` §7. Emits byte-compatible KiCad netlist format so
//! downstream tools (KiCad CvPcb, kinet2pcb, legacy CAM) don't know Signex
//! was the emitter.

use thiserror::Error;

use crate::{ExportContext, Exporter};

mod kicad_sexpr;
use kicad_sexpr::{build_net_graph, emit_comp, emit_net, emit_header};

pub struct NetlistExporter;

#[derive(Debug, Clone, Default)]
pub struct NetlistOptions {
    pub include_timestamps: bool,
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

        // Build net graph from first sheet (TODO: handle multi-sheet properly)
        let graph = if let Some(first_sheet) = ctx.sheets.first() {
            build_net_graph(&first_sheet.schematic, &all_symbols.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>())
        } else {
            return Err(NetlistError::GraphConstruction);
        };

        // Emit components
        for (sym, sheet_snap) in &all_symbols {
            if let Some(lib_sym) = sheet_snap.schematic.lib_symbols.get(&sym.lib_id) {
                let sheet_path = format!("/{}/", sheet_snap.sheet_name);
                let sheet_tstamp = format!("/<{:08x}>/", sheet_snap.sheet_number);

                let comp = emit_comp(sym, lib_sym, &sheet_path, &sheet_tstamp, opts.include_timestamps);
                components.push(comp);
            }
        }

        // Build nets with auto-generated names
        let mut net_code = 1u32;
        let mut nets_sexpr = Vec::new();

        // Get all root indices and sort them for deterministic output
        let mut roots: Vec<usize> = graph.net_names.keys().copied().collect();
        roots.sort();

        for root_idx in roots {
            let name = graph.net_names
                .get(&root_idx)
                .cloned()
                .unwrap_or_else(|| format!("Net-({})", net_code));

            if let Some(pins) = graph.node_to_pins.get(&root_idx) {
                let net = emit_net(net_code, &name, pins);
                nets_sexpr.push(net);
                net_code += 1;
            }
        }

        // Build the root export node
        let tool_version = "Signex 0.8.0";
        let timestamp = "2000-01-01T00:00:00".to_string();

        let source = ctx.sheets
            .first()
            .map(|s| s.path.display().to_string())
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
        let header = emit_header("/path/to/test.kicad_sch", "2026-04-22T00:00:00", "Signex 0.8.0");
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

        assert!(output.contains("(name \"VCC_3V3\")"), "Expected VCC_3V3 net name in output");
    }

    fn build_test_context_with_2_symbols() -> ExportContext {
        // Minimal test context with 2 symbols
        use uuid::Uuid;
        use signex_types::schematic::{SchematicSheet, Symbol, Point, LibSymbol, LibPin, Pin, PinElectricalType, PinShape};
        use std::collections::HashMap;

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
                        pin_type: PinElectricalType::Passive,
                        shape: PinShape::Line,
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
                        pin_type: PinElectricalType::Passive,
                        shape: PinShape::Line,
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
        use uuid::Uuid;
        use signex_types::schematic::{SchematicSheet, Symbol, Point};

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
        use uuid::Uuid;
        use signex_types::schematic::{SchematicSheet, Symbol, Point, Wire, Label, LabelType, LibSymbol, LibPin, Pin, PinElectricalType, PinShape};
        use std::collections::HashMap;

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
                        pin_type: PinElectricalType::Passive,
                        shape: PinShape::Line,
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
            ],
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
}
