//! References as the project's designator standard shows them (#1416).
//!
//! Under EN IEC 81346-2 full class codes a reference is stored as `QAC1` and
//! printed as `-QAC1`. The app applies this once to the export copy, so every
//! exporter (PDF, SVG, BOM, netlist, ERC report) prints the same text without
//! knowing about standards. The document is never touched.

use signex_types::designator::{DesignatorStandard, display_reference};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::SchematicSheet;

use crate::ExportContext;

impl ExportContext {
    /// Rewrite every reference the exporters print, on the pages, on the
    /// netlist's sheets and on the netlist's terminals alike, so a join from
    /// a terminal back to its symbol still matches. A standard that adds no
    /// sign (or none) changes nothing.
    pub fn show_references_for(&mut self, standard: Option<DesignatorStandard>) {
        if !standard.is_some_and(DesignatorStandard::shows_product_aspect_sign) {
            return;
        }
        for page in &mut self.sheets {
            show_sheet_references(&mut page.schematic, standard);
        }
        for sheet in &mut self.netlist_sheets {
            show_sheet_references(sheet, standard);
        }
        let terminals = self
            .netlist
            .iter_mut()
            .flat_map(|netlist| netlist.nets.iter_mut())
            .flat_map(|net| net.terminals.iter_mut());
        for terminal in terminals {
            terminal.reference = display_reference(standard, &terminal.reference).into_owned();
        }
    }
}

/// The board-output copy of [`ExportContext::show_references_for`]: every
/// footprint's reference and its designator text (the `Reference`
/// property) as `standard` shows them, so the board PDF, the reports and
/// the fabrication files (centroid, IPC-D-356, legend text) print `-QAC1`.
/// The caller passes a copy of the board, never the document's own.
pub fn show_board_references(board: &mut PcbBoard, standard: Option<DesignatorStandard>) {
    if !standard.is_some_and(DesignatorStandard::shows_product_aspect_sign) {
        return;
    }
    let key = pcb_geom::BoardTextField::Designator.key();
    for footprint in &mut board.footprints {
        footprint.reference = display_reference(standard, &footprint.reference).into_owned();
        for property in footprint.properties.iter_mut().filter(|p| p.key == key) {
            property.value = display_reference(standard, &property.value).into_owned();
        }
    }
}

fn show_sheet_references(sheet: &mut SchematicSheet, standard: Option<DesignatorStandard>) {
    for symbol in &mut sheet.symbols {
        symbol.reference = display_reference(standard, &symbol.reference).into_owned();
        for instance in &mut symbol.instances {
            instance.reference = display_reference(standard, &instance.reference).into_owned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectMetadata, SheetSnapshot};
    use signex_types::net::{Net, NetId, Netlist, Terminal};
    use signex_types::schematic::{Symbol, SymbolInstance};

    fn context(references: &[&str]) -> ExportContext {
        let symbols = references
            .iter()
            .map(|reference| Symbol {
                reference: (*reference).to_owned(),
                instances: vec![SymbolInstance {
                    reference: (*reference).to_owned(),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .collect();
        let sheet = SchematicSheet {
            symbols,
            ..Default::default()
        };
        let terminals = references
            .iter()
            .map(|reference| Terminal {
                symbol: uuid::Uuid::nil(),
                reference: (*reference).to_owned(),
                pin: "1".into(),
                occurrence: Default::default(),
            })
            .collect();
        ExportContext {
            sheets: vec![SheetSnapshot {
                path: "a.snxsch".into(),
                schematic: sheet.clone(),
                sheet_name: "a".into(),
                sheet_number: 1,
                sheet_count: 1,
                placement: Default::default(),
            }],
            metadata: ProjectMetadata::default(),
            netlist: Some(Netlist {
                nets: vec![Net {
                    id: NetId(0),
                    name: "N".into(),
                    class: None,
                    wires: Vec::new(),
                    junctions: Vec::new(),
                    terminals,
                }],
            }),
            netlist_sheets: vec![sheet],
            netlist_placements: Vec::new(),
            current_sheet: None,
        }
    }

    fn printed(ctx: &ExportContext) -> Vec<Vec<String>> {
        let page = &ctx.sheets[0].schematic.symbols;
        let netlist_sheet = &ctx.netlist_sheets[0].symbols;
        let terminals = &ctx.netlist.as_ref().unwrap().nets[0].terminals;
        vec![
            texts(page.iter().map(|s| &s.reference)),
            texts(page.iter().map(|s| &s.instances[0].reference)),
            texts(netlist_sheet.iter().map(|s| &s.reference)),
            texts(terminals.iter().map(|t| &t.reference)),
        ]
    }

    fn texts<'a>(references: impl Iterator<Item = &'a String>) -> Vec<String> {
        references.cloned().collect()
    }

    #[test]
    fn full_class_codes_print_with_the_product_aspect_sign_everywhere() {
        let mut ctx = context(&["QAC1", "#PWR1", "RAC2"]);
        ctx.show_references_for(Some(DesignatorStandard::Iec81346_2Full));
        for references in printed(&ctx) {
            assert_eq!(references, ["-QAC1", "#PWR1", "-RAC2"]);
        }
    }

    #[test]
    fn other_standards_print_the_stored_reference() {
        for standard in [
            None,
            Some(DesignatorStandard::Ieee315),
            Some(DesignatorStandard::Iec81346_2),
        ] {
            let mut ctx = context(&["Q1", "R2"]);
            ctx.show_references_for(standard);
            for references in printed(&ctx) {
                assert_eq!(references, ["Q1", "R2"]);
            }
        }
    }
}
