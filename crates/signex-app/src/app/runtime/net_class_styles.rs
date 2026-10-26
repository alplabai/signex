//! Keeps the canvas's resolved net-class styles current.

use signex_types::net_class::NetClass;

use crate::app::state::net_class_styles::SheetClassInput;
use crate::app::state::scope;

use super::Signex;

impl Signex {
    /// Re-resolve the net-class style of every open sheet after an update.
    ///
    /// The join of the project's class table and each sheet's netlist is
    /// rebuilt only for a sheet whose engine revision or class table changed.
    /// When anything drawn changed, the schematic canvases drop their content
    /// caches so the next frame repaints with the new style. A session with
    /// no classes anywhere returns before gathering any input.
    pub(crate) fn refresh_net_class_styles(&mut self) {
        let document = &self.document_state;
        let any_classes = document
            .projects
            .iter()
            .any(|project| !project.data.net_classes.is_empty());
        if !any_classes && self.ui_state.net_class_styles.is_empty() {
            return;
        }
        let refs = document.child_sheet_refs();
        let inputs = document.engines.iter().map(|(path, engine)| {
            let classes: &[NetClass] = scope::project_owning_sheet(&document.projects, &refs, path)
                .map(|project| project.data.net_classes.as_slice())
                .unwrap_or(&[]);
            SheetClassInput {
                path,
                revision: engine.revision(),
                sheet: engine.document(),
                classes,
            }
        });
        if self.ui_state.net_class_styles.refresh(inputs) {
            for canvas in self.interaction_state.all_schematic_canvases_mut() {
                canvas.clear_content_cache();
            }
        }
    }
}
