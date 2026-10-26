//! The Net Class picker of the selected net label.
//!
//! A class is keyed by the net's name in the project netlist, which is not the
//! label's own text on a child sheet (`LOCAL` there is the net `/LOCAL`) and
//! can be another label's or a power port's name. So the picker resolves the
//! net from the project's resolved netlist, through the wire the label is
//! attached to, and targets the project the label's own sheet belongs to,
//! never whichever project happens to be active.

use signex_types::schematic::SelectedKind;

use crate::panels::LabelClassPick;

use super::Signex;

impl Signex {
    /// Keep the panel context's picker in step with the selection. A
    /// selection change does not always rebuild the whole panel context, so
    /// every update ends here; it clones nothing unless a label is selected.
    pub(crate) fn sync_label_class_pick(&mut self) {
        let pick = self.selected_label_class_pick();
        if self.document_state.panel_ctx.net_classes.label != pick {
            self.document_state.panel_ctx.net_classes.label = pick;
        }
    }

    /// The picker for the single selected label, or `None` when no label is
    /// selected, its sheet belongs to no project, the project has no class
    /// table to pick from, or the project netlist has not resolved the
    /// label's net yet (the picker then appears once it has).
    pub(crate) fn selected_label_class_pick(&self) -> Option<LabelClassPick> {
        let document = &self.document_state;
        let ctx = &document.panel_ctx;
        if ctx.selected_kind != Some(SelectedKind::Label) {
            return None;
        }
        let label_id = ctx.selected_uuid?;
        let path = document.active_path.as_deref()?;
        let project = document.document_project(path)?;
        if project.data.net_classes.is_empty() {
            return None;
        }
        let sheet = document.engines.get(path)?.document();
        let label = sheet.labels.iter().find(|l| l.uuid == label_id)?;
        let wire = signex_net::wire_at_point(sheet, &label.position)?;
        let net = self.ui_state.net_class_styles.net_of(project.id, wire)?;
        Some(LabelClassPick {
            project: project.id,
            net: net.to_string(),
            classes: project.data.net_classes.clone(),
        })
    }
}
