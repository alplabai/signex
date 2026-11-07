//! The `assign_net_class` command (#661, #1934): the label picker's
//! `NetClassMsg::AssignNet`, run from operands, so a pick has one behaviour
//! on every surface. It acts on the document it is given and never touches
//! the active tab, focus or panels.

use std::path::Path;

use crate::panels::NetClassMsg;

impl crate::app::Signex {
    /// Put `net` (its name in the project netlist) in `class`, or in no class
    /// for `None`, in the project the open sheet at `sheet` belongs to (the
    /// active sheet when `None`). The pick lands on that sheet's timeline.
    /// `Err` with the reason when nothing was assigned.
    pub(crate) fn assign_net_to_class(
        &mut self,
        sheet: Option<&Path>,
        net: &str,
        class: Option<&str>,
    ) -> Result<(), String> {
        let document = &self.document_state;
        let path = sheet
            .or(document.active_path.as_deref())
            .ok_or("there is no active sheet")?
            .to_path_buf();
        if !document.engines.contains_key(&path) {
            return Err(format!("{} is not an open sheet", path.display()));
        }
        let project = document
            .document_project(&path)
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
        self.handle_net_class_message_on(
            Some(&path),
            &NetClassMsg::AssignNet {
                project: id,
                net: net.to_string(),
                class: class.map(str::to_string),
            },
        );
        let changed = self
            .document_state
            .projects
            .iter()
            .any(|p| p.id == id && p.data.net_classes != before);
        match (changed, class) {
            (true, _) => Ok(()),
            (false, Some(_)) => Err(format!("\"{net}\" is already in that class")),
            (false, None) => Err(format!("\"{net}\" already has no class")),
        }
    }
}
