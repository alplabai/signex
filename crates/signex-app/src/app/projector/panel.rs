//! The visible document's counts and paper, written into the panel context.

use crate::app::Signex;

impl Signex {
    pub(crate) fn sync_panel_ctx_from_visible_document(&mut self) {
        self.document_view.panel_ctx.has_schematic = self.has_active_schematic();
        self.document_view.panel_ctx.has_pcb = self.has_active_pcb();

        // Counts and paper label for whichever document is visible; the
        // per-row symbol lists come from the same builder `refresh_panel_ctx`
        // uses, so the two paths cannot disagree about what a row holds.
        let (counts, paper_size) = if let Some(snapshot) = self.active_render_snapshot() {
            (
                (
                    snapshot.symbols.len(),
                    snapshot.wires.len(),
                    snapshot.labels.len(),
                    snapshot.junctions.len(),
                    snapshot.lib_symbols.len(),
                ),
                // Older documents may carry an empty paper_size (serde
                // default); normalize so the Page Options selector and the
                // drawn sheet agree on the effective A4 fallback.
                effective_paper_size(&snapshot.paper_size),
            )
        } else if let Some(snapshot) = self.active_pcb_snapshot() {
            (
                (
                    snapshot.footprints.len(),
                    snapshot.segments.len(),
                    snapshot.texts.len(),
                    snapshot.vias.len(),
                    0,
                ),
                format!("PCB • {} layers", snapshot.layers.len()),
            )
        } else {
            ((0, 0, 0, 0, 0), "A4".to_string())
        };

        {
            let ctx = &mut self.document_view.panel_ctx;
            ctx.sym_count = counts.0;
            ctx.wire_count = counts.1;
            ctx.label_count = counts.2;
            ctx.junction_count = counts.3;
            ctx.lib_symbol_count = counts.4;
            ctx.paper_size = paper_size;
        }
        self.fill_symbol_lists(None);
        // Written outside `fill_symbol_lists`; the next refresh must rebuild.
        self.document_view.panel_ctx.symbol_lists_key = None;

        // Drive the drawn sheet from the (re)loaded document's paper size —
        // without this the canvas keeps the previous tab's dimensions and the
        // sheet rectangle stops matching the stored A-series format.
        if self.has_active_schematic() {
            self.apply_page_dimensions_to_canvas();
        }
    }
}

/// Paper token for the panel context: older documents carry an empty
/// `paper_size` (serde default), which every reader treats as A4.
pub(crate) fn effective_paper_size(paper_size: &str) -> String {
    if paper_size.is_empty() {
        "A4".to_string()
    } else {
        paper_size.to_string()
    }
}

#[cfg(test)]
mod effective_paper_size_tests {
    use super::effective_paper_size;

    #[test]
    fn an_empty_paper_size_reads_as_a4_everywhere() {
        assert_eq!(effective_paper_size(""), "A4");
        assert_eq!(effective_paper_size("A3"), "A3");
    }
}
