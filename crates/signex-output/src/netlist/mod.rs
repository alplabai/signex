//! Netlist export.
//!
//! The Standard-format `.net` S-expression emitter that previously lived
//! here was split out as part of the issue #62 Apache-clean cutover.
//! It moves to the optional `signex-standard-import` GPL-3.0 companion
//! repository alongside the rest of the Standard I/O codepaths.
//!
//! Future Signex-native netlist formats (XML, Spice, etc.) will land
//! here as separate `Exporter` impls. The exported types
//! (`NetlistExporter`, `NetlistOptions`, `NetlistOutput`) stay so the
//! app-layer wiring keeps compiling; the exporter currently returns
//! `NetlistError::NotImplemented` to surface the migration to the user.

use thiserror::Error;

use crate::{ExportContext, Exporter};

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
    #[error("netlist export is not yet available in Signex Community; the Standard-format emitter has moved to the signex-standard-import companion repo (issue #62)")]
    NotImplemented,
}

impl Exporter for NetlistExporter {
    type Options = NetlistOptions;
    type Output = NetlistOutput;
    type Error = NetlistError;

    fn export(
        &self,
        _ctx: &ExportContext,
        _opts: &Self::Options,
    ) -> Result<Self::Output, Self::Error> {
        Err(NetlistError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_valid_header() {
        let header = emit_header(
            "/path/to/test.standard_sch",
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
            LibPin, LibSymbol, Pin, PinElectricalType, PinShape, Point, SchematicSheet, Symbol,
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            path: std::path::PathBuf::from("test.standard_sch"),
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            path: std::path::PathBuf::from("test.standard_sch"),
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
            Label, LabelType, LibPin, LibSymbol, Pin, PinElectricalType, PinShape, Point,
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            path: std::path::PathBuf::from("test.standard_sch"),
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
            LibPin, LibSymbol, Pin, PinElectricalType, PinShape, Point, SchematicSheet, Symbol,
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            path: std::path::PathBuf::from("test.standard_sch"),
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
            Label, LabelType, LibPin, LibSymbol, Pin, PinElectricalType, PinShape, Point,
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
            library_id: None,
            row_id: None,
            library_version: String::new(),
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
            path: std::path::PathBuf::from("test.standard_sch"),
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
            path: std::path::PathBuf::from("test1.standard_sch"),
            schematic: sheet1,
            sheet_name: "Sheet1".to_string(),
            sheet_number: 1,
            sheet_count: 2,
        };

        let snap2 = crate::SheetSnapshot {
            path: std::path::PathBuf::from("test2.standard_sch"),
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
