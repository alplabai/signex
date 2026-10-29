//! Jumping between sheets: to a port's partner, back and forward through the
//! sheets visited, and the shared "go to this member" step.

use iced::Task;
use signex_net::{NetMember, NetMemberKind};
use signex_types::schematic::{SelectedItem, SelectedKind};

use crate::app::state::net_nav::{PendingNetAction, ProjectNetData};

use super::super::super::*;
use super::index_job::IndexLookup;
use super::target::NetTarget;

/// The selection kind a net member is picked as on its sheet.
pub(super) fn selected_kind_of(member: &NetMember) -> SelectedKind {
    match member.kind {
        NetMemberKind::Wire => SelectedKind::Wire,
        NetMemberKind::Junction => SelectedKind::Junction,
        NetMemberKind::Pin { .. } | NetMemberKind::PowerPort => SelectedKind::Symbol,
        NetMemberKind::Label(_) => SelectedKind::Label,
        NetMemberKind::SheetPin { .. } => SelectedKind::SheetPin,
    }
}

/// A member in words, for the Messages panel: `R1.2`, a label's text, a sheet
/// entry's name.
pub(super) fn describe_member(member: &NetMember) -> String {
    match &member.kind {
        NetMemberKind::Pin { reference, pin } => format!("{reference}.{pin}"),
        NetMemberKind::Wire => "a wire".to_string(),
        NetMemberKind::Junction => "a junction".to_string(),
        NetMemberKind::PowerPort | NetMemberKind::Label(_) | NetMemberKind::SheetPin { .. } => {
            member.text.clone()
        }
    }
}

impl Signex {
    /// Go to net partner: from the selected (else pointed-at) port, global
    /// label, hierarchical label or sheet entry to the next one on its net,
    /// cycling when there are several.
    pub(crate) fn jump_to_partner(&mut self) -> Task<Message> {
        let Some(target) = self.port_target() else {
            crate::diagnostics::log_info(
                "Select or point at a port, global label, hierarchical label or sheet entry, then run Go to net partner.",
            );
            return Task::none();
        };
        let Some(project) = self.project_of_sheet(&target.sheet) else {
            crate::diagnostics::log_info(
                "Go to net partner needs the sheet to belong to an open project; open its .snxprj.",
            );
            return Task::none();
        };
        match self.net_index_lookup(project) {
            IndexLookup::Ready(data) => self.jump_with(&data, &target),
            IndexLookup::Building(build) => {
                self.wait_for_net_index(PendingNetAction::JumpToPartner, build)
            }
            IndexLookup::Unavailable => {
                crate::diagnostics::log_warning(
                    "Net navigation is unavailable for this project; see the earlier message.",
                );
                Task::none()
            }
        }
    }

    /// The selection, else the pointed-at object, when it is a label or sheet
    /// entry.
    fn port_target(&self) -> Option<NetTarget> {
        let is_port_kind =
            |t: &NetTarget| matches!(t.item.kind, SelectedKind::Label | SelectedKind::SheetPin);
        self.target_from_selection()
            .filter(is_port_kind)
            .or_else(|| self.target_under_cursor().filter(is_port_kind))
    }

    fn jump_with(&mut self, data: &ProjectNetData, target: &NetTarget) -> Task<Message> {
        let Some(key) = data.key_of(&target.sheet) else {
            crate::diagnostics::log_warning(
                "Go to net partner: this sheet is not in the project's hierarchy; add it to the project.",
            );
            return Task::none();
        };
        let Some(source) = data
            .index
            .member(&key, target.item.uuid)
            .filter(|m| m.is_port())
        else {
            crate::diagnostics::log_info(
                "That label is not a port. Use a global label, hierarchical label or sheet entry.",
            );
            return Task::none();
        };
        let name = describe_member(source);
        let partners = data.index.partners(&key, target.item.uuid);
        let Some(first) = partners.first().copied() else {
            crate::diagnostics::log_info(format!(
                "No partner for '{name}' on another sheet. Add a matching global label or sheet entry."
            ));
            return Task::none();
        };
        let total = partners.len();
        let (member, sheet) = (first.clone(), data.sheet_label(&first.sheet));
        let Some(path) = data
            .path_of(&member.sheet)
            .map(std::path::Path::to_path_buf)
        else {
            crate::diagnostics::log_warning(format!(
                "Partner of '{name}' is on '{sheet}', which has no file. Check the project's sheet list."
            ));
            return Task::none();
        };
        let Some(task) = self.go_to_member(&path, &member) else {
            return Task::none();
        };
        crate::diagnostics::log_info(format!(
            "'{name}': partner 1 of {total} on {sheet}. Run Go to net partner again for the next."
        ));
        task
    }

    /// Open `path` at `member`'s placement, select the member and centre on
    /// it. `None` when the sheet could not be opened (said in Messages).
    pub(super) fn go_to_member(
        &mut self,
        path: &std::path::Path,
        member: &NetMember,
    ) -> Option<Task<Message>> {
        let held = self.resolve_held_spelling(path.to_path_buf());
        if self.document_state.active_path.as_deref() != Some(held.as_path()) {
            self.open_sheet_with_placement(path.to_path_buf(), Some(member.occurrence.clone()));
        }
        let opened = self.resolve_held_spelling(path.to_path_buf());
        if self.document_state.active_path.as_deref() != Some(opened.as_path()) {
            crate::diagnostics::log_warning(format!(
                "Could not open {}. Check that the file exists and is a valid sheet.",
                path.display()
            ));
            return None;
        }
        let item = SelectedItem::new(member.id, selected_kind_of(member));
        Some(self.handle_focus_at(member.at.x, member.at.y, Some(item)))
    }

    /// Back / forward through the sheets visited. A sheet that can no longer
    /// be opened is skipped and said so.
    pub(crate) fn step_sheet_history(&mut self, forward: bool) -> Task<Message> {
        loop {
            let history = &mut self.document_state.net_nav.history;
            let step = if forward {
                history.forward()
            } else {
                history.back()
            };
            let Some(path) = step else {
                let edge = if forward { "later" } else { "earlier" };
                crate::diagnostics::log_info(format!(
                    "No {edge} sheet in the history. Open or jump to another sheet first."
                ));
                return Task::none();
            };
            if self.open_or_focus_sheet_path(path.clone()) {
                return Task::none();
            }
            crate::diagnostics::log_warning(format!(
                "Sheet history skipped {}: it cannot be opened.",
                path.display()
            ));
        }
    }

    /// A schematic sheet came into focus: record it as the current one in the
    /// history, and show the net highlight the way that sheet sees it.
    pub(crate) fn note_sheet_focused(&mut self) {
        if let Some(tab) = self.document_state.tabs.get(self.document_state.active_tab)
            && matches!(tab.kind, TabKind::Schematic)
        {
            let path = tab.path.clone();
            self.document_state.net_nav.history.visit(&path);
        }
        self.sync_net_highlight_to_canvases();
    }
}
