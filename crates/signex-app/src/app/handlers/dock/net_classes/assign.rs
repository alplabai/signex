//! The `assign_net_class` command (#661, #1934): the label picker's
//! `NetClassMsg::AssignNet`, run from operands, so a pick has one behaviour
//! on every surface (the marker lands on the active sheet's timeline).

use crate::panels::NetClassMsg;

impl crate::app::Signex {
    /// Put `net` (its name in the project netlist) in `class`, or in no class
    /// for `None`, in the project the active sheet belongs to. `Err` with the
    /// reason when nothing was assigned.
    pub(crate) fn assign_net_to_class(
        &mut self,
        net: &str,
        class: Option<&str>,
    ) -> Result<(), String> {
        let document = &self.document_state;
        let path = document
            .active_path
            .as_deref()
            .ok_or("there is no active sheet")?;
        let project = document
            .document_project(path)
            .ok_or("the sheet belongs to no project")?;
        if project.data.net_classes.is_empty() {
            return Err("the project has no net classes".into());
        }
        if let Some(class) = class
            && !project.data.net_classes.iter().any(|c| c.name == class)
        {
            return Err(format!("the project has no net class \"{class}\""));
        }
        if !self.ui_state.net_class_styles.knows_net(project.id, net) {
            return Err(format!(
                "\"{net}\" is not a net of the project netlist (or it is not resolved yet)"
            ));
        }
        let (id, before) = (project.id, project.data.net_classes.clone());
        self.handle_net_class_message(&NetClassMsg::AssignNet {
            project: id,
            net: net.to_string(),
            class: class.map(str::to_string),
        });
        let changed = self
            .document_state
            .projects
            .iter()
            .any(|p| p.id == id && p.data.net_classes != before);
        if changed {
            Ok(())
        } else {
            Err(format!("\"{net}\" is already in that class"))
        }
    }
}
