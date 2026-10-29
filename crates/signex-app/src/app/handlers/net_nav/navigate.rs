//! Jumping between sheets: to a port's partner, back and forward through the
//! sheets visited, and the shared "go to this member" step.

use iced::Task;
use signex_net::{NetMember, NetMemberKind};
use signex_types::schematic::{SelectedItem, SelectedKind};

use std::sync::Arc;

use crate::app::state::net_nav::{JumpCursor, PendingNetAction, ProjectNetData};

use super::super::super::*;
use super::index_job::IndexLookup;
use super::messages;
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
            crate::diagnostics::log_info(messages::no_port_selected());
            return Task::none();
        };
        let Some(project) = self.project_of_sheet(&target.sheet) else {
            crate::diagnostics::log_info(messages::no_project("Go to net partner"));
            return Task::none();
        };
        // The user asked: a build that failed before may work now.
        self.document_state.net_nav.index.clear_failed(project);
        match self.net_index_lookup(project) {
            IndexLookup::Ready(data) => self.jump_with(&data, &target),
            IndexLookup::Building(build) => {
                self.wait_for_net_index(project, PendingNetAction::JumpToPartner, build)
            }
            IndexLookup::Unavailable => {
                crate::diagnostics::log_warning(messages::index_unavailable());
                Task::none()
            }
        }
    }

    /// The selection, else the pointed-at object, when it is a label or sheet
    /// entry.
    fn port_target(&self) -> Option<NetTarget> {
        // A placed symbol is a candidate for its power port; the index says
        // whether it is one.
        let is_port_kind = |t: &NetTarget| {
            matches!(
                t.item.kind,
                SelectedKind::Label | SelectedKind::SheetPin | SelectedKind::Symbol
            )
        };
        self.target_from_selection()
            .filter(is_port_kind)
            .or_else(|| self.target_under_cursor().filter(is_port_kind))
    }

    fn jump_with(&mut self, data: &Arc<ProjectNetData>, target: &NetTarget) -> Task<Message> {
        let Some(key) = data.key_of(&target.sheet) else {
            crate::diagnostics::log_warning(messages::sheet_not_in_project());
            return Task::none();
        };
        let placement = self.net_occurrence(data, &target.sheet);
        let occurrence = placement.as_ref();
        let member = data.index.member(&key, target.item.uuid, occurrence);
        let Some(source) = member.filter(|m| m.is_port()) else {
            let name = member.map(describe_member).unwrap_or_default();
            crate::diagnostics::log_info(messages::not_a_port(&name));
            return Task::none();
        };
        let name = describe_member(source);
        // Carry on the lap this jump continues, or start one from this port.
        let anchor = (key.clone(), target.item.uuid, source.occurrence.clone());
        let nav = &mut self.document_state.net_nav;
        if !nav
            .jump
            .as_ref()
            .is_some_and(|c| c.continues(data, &anchor))
        {
            let partners: Vec<NetMember> = data
                .index
                .jump_ring(&key, target.item.uuid, occurrence)
                .into_iter()
                .cloned()
                .collect();
            if partners.is_empty() {
                nav.jump = None;
                crate::diagnostics::log_info(messages::no_partner(&name));
                return Task::none();
            }
            nav.jump = Some(JumpCursor::start(
                Arc::clone(data),
                source.clone(),
                partners,
            ));
        }
        let Some(cursor) = nav.jump.as_mut() else {
            return Task::none();
        };
        let total = cursor.lap_len() - 1;
        let first = cursor.advance().clone();
        let (member, sheet) = (first.clone(), data.sheet_label(&first.sheet));
        let Some(path) = data
            .path_of(&member.sheet)
            .map(std::path::Path::to_path_buf)
        else {
            crate::diagnostics::log_warning(messages::partner_has_no_file(&name, &sheet));
            return Task::none();
        };
        let Some(task) = self.go_to_member(data, &path, &member) else {
            return Task::none();
        };
        crate::diagnostics::log_info(messages::jumped(&name, total, &sheet));
        task
    }

    /// Open `path` at `member`'s placement, select the member and centre on
    /// it. `None` when the sheet could not be opened (said in Messages).
    pub(super) fn go_to_member(
        &mut self,
        data: &ProjectNetData,
        path: &std::path::Path,
        member: &NetMember,
    ) -> Option<Task<Message>> {
        let held = self.resolve_held_spelling(path.to_path_buf());
        // Another placement of the same file is another sheet instance: switch
        // to the member's own, not just to the file.
        let shows_it = self.net_occurrence(data, &held).as_ref() == Some(&member.occurrence);
        if self.document_state.active_path.as_deref() != Some(held.as_path()) || !shows_it {
            self.open_sheet_with_placement(path.to_path_buf(), Some(member.occurrence.clone()));
        }
        let opened = self.resolve_held_spelling(path.to_path_buf());
        if self.document_state.active_path.as_deref() != Some(opened.as_path()) {
            crate::diagnostics::log_warning(messages::cannot_open(path));
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
            let Some(path) = history.peek(forward).map(std::path::Path::to_path_buf) else {
                crate::diagnostics::log_info(messages::no_history(forward));
                return Task::none();
            };
            // A sheet that is neither open nor on disk is skipped here, so the
            // open path does not report it as a missing child sheet.
            let open = self.document_state.tabs.iter().any(|t| t.path == path);
            if open || path.exists() {
                // The history moves only once the sheet is really shown, and
                // the focus change this causes is not a visit of its own.
                self.document_state.net_nav.history.hold(true);
                let shown = self.open_or_focus_sheet_path(path.clone());
                self.document_state.net_nav.history.hold(false);
                if shown {
                    let landed = self.active_tab_path().unwrap_or(path);
                    self.document_state.net_nav.history.arrive(forward, &landed);
                    return Task::none();
                }
            }
            // It cannot be opened: forget it rather than skip it every pass.
            self.document_state.net_nav.history.drop_entry(forward);
            crate::diagnostics::log_warning(messages::history_skipped(&path));
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
        // A card read on the sheet just left is stale.
        self.document_state.net_nav.hover = Default::default();
    }
}
