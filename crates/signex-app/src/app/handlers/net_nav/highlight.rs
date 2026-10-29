//! The persistent net highlight: set it from a point or the selection, step
//! through the net's members across sheets, clear it, and keep what each
//! canvas draws bright in step with it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::Task;
use signex_net::NetMemberKind;
use signex_types::schematic::SelectedItem;

use crate::app::state::net_nav::{NetHighlight, PendingNetAction, ProjectNetData};
use crate::app::state::{ProjectId, WindowKind};

use super::super::super::*;
use super::index_job::IndexLookup;
use super::messages;
use super::navigate::{describe_member, selected_kind_of};
use super::target::{NetTarget, net_of_target};

/// The elements of net `net` on each sheet file, for what stays bright.
fn bright_elements(
    data: &ProjectNetData,
    net: usize,
) -> HashMap<PathBuf, Arc<HashSet<uuid::Uuid>>> {
    data.index
        .highlight_elements(net)
        .into_iter()
        .filter_map(|(key, ids)| Some((data.path_of(&key)?.to_path_buf(), Arc::new(ids))))
        .collect()
}

impl Signex {
    /// Highlight the net under the cursor on every sheet (else the selection's).
    pub(crate) fn highlight_net_under_cursor(&mut self) -> Task<Message> {
        let target = self
            .target_under_cursor()
            .or_else(|| self.target_from_selection());
        self.highlight_target(target, PendingNetAction::HighlightUnderCursor, false)
    }

    /// Highlight the selection's net (else the cursor's), and select what is
    /// on it on this sheet.
    pub(crate) fn highlight_related_net_objects(&mut self) -> Task<Message> {
        let target = self
            .target_from_selection()
            .or_else(|| self.target_under_cursor());
        self.highlight_target(target, PendingNetAction::HighlightRelated, true)
    }

    fn highlight_target(
        &mut self,
        target: Option<NetTarget>,
        action: PendingNetAction,
        select_here: bool,
    ) -> Task<Message> {
        let Some(target) = target else {
            crate::diagnostics::log_info(messages::nothing_pointed_at());
            return Task::none();
        };
        let Some(project) = self.project_of_sheet(&target.sheet) else {
            crate::diagnostics::log_info(messages::no_project("net highlight"));
            return Task::none();
        };
        match self.net_index_lookup(project) {
            IndexLookup::Ready(data) => {
                self.highlight_with(project, &data, &target, select_here);
                Task::none()
            }
            IndexLookup::Building(build) => self.wait_for_net_index(action, build),
            IndexLookup::Unavailable => {
                crate::diagnostics::log_warning(messages::index_unavailable());
                Task::none()
            }
        }
    }

    fn highlight_with(
        &mut self,
        project: ProjectId,
        data: &ProjectNetData,
        target: &NetTarget,
        select_here: bool,
    ) {
        let net = data.key_of(&target.sheet).and_then(|key| {
            let occurrence = target.occurrence.as_ref();
            let net = net_of_target(data, &key, &target.item, target.at, occurrence)?;
            Some((key, net))
        });
        let Some((key, net)) = net else {
            crate::diagnostics::log_info(messages::on_no_net());
            return;
        };
        let name = data
            .index
            .net(net)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let elements = bright_elements(data, net);
        let sheets = elements.len();
        self.document_state.net_nav.highlight = Some(NetHighlight {
            project,
            name: name.clone(),
            anchor: (key.clone(), target.item.uuid),
            cursor: None,
            elements,
        });
        if select_here {
            self.select_net_members_here(data, &key, net, target.occurrence.as_ref());
        }
        self.sync_net_highlight_to_canvases();
        crate::diagnostics::log_info(messages::highlighted(&name, sheets));
    }

    /// Select the members of `net` that sit on the focused sheet.
    fn select_net_members_here(
        &mut self,
        data: &ProjectNetData,
        key: &signex_net::SheetKey,
        net: usize,
        occurrence: Option<&signex_types::schematic::InstancePath>,
    ) {
        let Some(members) = data.index.net(net).map(|n| &n.members) else {
            return;
        };
        let mut items: Vec<SelectedItem> = Vec::new();
        for member in members
            .iter()
            .filter(|m| &m.sheet == key && occurrence.is_none_or(|o| &m.occurrence == o))
        {
            let item = SelectedItem::new(member.id, selected_kind_of(member));
            if !items.contains(&item) {
                items.push(item);
            }
        }
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.selected = items;
        canvas.clear_overlay_cache();
        self.update_selection_info();
    }

    /// Step to the next or previous member of the highlighted net, across
    /// sheets, opening each sheet it is on.
    pub(crate) fn step_net_highlight(&mut self, forward: bool) -> Task<Message> {
        let Some(project) = self
            .document_state
            .net_nav
            .highlight
            .as_ref()
            .map(|h| h.project)
        else {
            crate::diagnostics::log_info(messages::nothing_highlighted());
            return Task::none();
        };
        match self.net_index_lookup(project) {
            IndexLookup::Ready(data) => self.step_with(&data, forward),
            IndexLookup::Building(build) => {
                self.wait_for_net_index(PendingNetAction::StepHighlight { forward }, build)
            }
            IndexLookup::Unavailable => {
                crate::diagnostics::log_warning(messages::index_unavailable());
                Task::none()
            }
        }
    }

    fn step_with(&mut self, data: &ProjectNetData, forward: bool) -> Task<Message> {
        let Some(highlight) = self.document_state.net_nav.highlight.as_mut() else {
            return Task::none();
        };
        let Some(net) = data.index.resolve(&highlight.name, &highlight.anchor) else {
            self.document_state.net_nav.highlight = None;
            self.sync_net_highlight_to_canvases();
            crate::diagnostics::log_info(messages::highlight_gone());
            return Task::none();
        };
        let steps = data.index.steps(net);
        let Some(at) = highlight.step(steps.len(), forward) else {
            crate::diagnostics::log_info(messages::net_has_no_members());
            return Task::none();
        };
        let (name, total) = (highlight.name.clone(), steps.len());
        let member = steps[at].clone();
        let sheet = data.sheet_label(&member.sheet);
        let what = describe_member(&member);
        let Some(path) = data
            .path_of(&member.sheet)
            .map(std::path::Path::to_path_buf)
        else {
            crate::diagnostics::log_warning(messages::net_sheet_has_no_file(&name, &sheet));
            return Task::none();
        };
        let Some(task) = self.go_to_member(&path, &member) else {
            return Task::none();
        };
        let kind = match member.kind {
            NetMemberKind::Pin { .. } => "pin",
            NetMemberKind::Label(_) => "label",
            NetMemberKind::SheetPin { .. } => "sheet entry",
            _ => "item",
        };
        crate::diagnostics::log_info(messages::stepped(&name, kind, &what, &sheet, at + 1, total));
        task
    }

    /// Remove the net highlight.
    pub(crate) fn clear_net_highlight(&mut self) -> Task<Message> {
        if self.document_state.net_nav.highlight.take().is_none() {
            crate::diagnostics::log_info(messages::nothing_to_clear());
            return Task::none();
        }
        self.sync_net_highlight_to_canvases();
        crate::diagnostics::log_info(messages::cleared());
        Task::none()
    }

    /// A rebuilt index re-resolves the highlighted net, so an edit that adds
    /// or removes members changes what is bright; a net that no longer exists
    /// drops its highlight.
    pub(super) fn refresh_highlight_from(&mut self, project: ProjectId, data: &ProjectNetData) {
        let Some(highlight) = self.document_state.net_nav.highlight.as_mut() else {
            return;
        };
        if highlight.project != project {
            return;
        }
        match data.index.resolve(&highlight.name, &highlight.anchor) {
            Some(net) => {
                highlight.elements = bright_elements(data, net);
                if let Some(n) = data.index.net(net) {
                    highlight.name.clone_from(&n.name);
                }
                // Anchor on something that still exists, so the next rebuild
                // does not depend on an element that may be gone by then.
                if let Some(anchor) = data.index.anchor_of(net) {
                    highlight.anchor = anchor;
                }
            }
            None => {
                self.document_state.net_nav.highlight = None;
                crate::diagnostics::log_info(messages::highlight_gone());
            }
        }
        self.sync_net_highlight_to_canvases();
    }

    /// Push what stays bright into every schematic canvas: the main one shows
    /// the focused sheet, an undocked window its own. A sheet the net does not
    /// reach (or of another project) shows everything dimmed.
    pub(crate) fn sync_net_highlight_to_canvases(&mut self) {
        let highlight = self.document_state.net_nav.highlight.as_ref();
        let main_path = self.document_state.active_path.clone();
        let mut per_window: Vec<(iced::window::Id, Option<std::path::PathBuf>)> = Vec::new();
        for id in self.interaction_state.canvases.keys() {
            let path = match self.ui_state.windows.get(id) {
                Some(WindowKind::UndockedTab { path, .. }) => Some(path.clone()),
                _ => None,
            };
            per_window.push((*id, path));
        }
        let bright = |path: Option<&std::path::Path>| {
            let highlight = highlight?;
            let path = path?;
            (self.project_of_sheet(path) == Some(highlight.project))
                .then(|| highlight.bright_on(path))
        };
        let main = bright(main_path.as_deref());
        let windows: Vec<_> = per_window
            .iter()
            .map(|(id, path)| (*id, bright(path.as_deref())))
            .collect();
        self.interaction_state.canvas.set_net_highlight(main);
        for (id, bright) in windows {
            if let Some(slot) = self.interaction_state.canvases.get_mut(&id) {
                slot.set_net_highlight(bright);
            }
        }
    }
}
