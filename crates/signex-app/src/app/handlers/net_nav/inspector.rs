//! The Net Inspector panel (#1269): its list, kept in step with the cached
//! project net index, and a member click, which goes to the member through
//! the same path a net step uses and highlights its net. The By Component
//! rows (#1274) are built in `component_rows.rs` from the same index.

use std::collections::HashMap;
use std::sync::Arc;

use iced::Task;
use signex_net::{NetAnchor, NetMemberKind};

use super::super::super::*;
use super::component_rows::{component_rows, pin_names};
use super::highlight::bright_elements;
use super::messages;
use super::navigate::describe_member;
use crate::app::runtime::net_class_styles::plan_projects;
use crate::app::state::ProjectId;
use crate::app::state::net_nav::{NetHighlight, ProjectNetData};
use crate::panels::net_inspector::{
    NetInspectorTab, NetInspectorView, NetListStatus, NetMemberRow, NetMemberRowKind, NetRow,
    preview,
};
use crate::panels::{NetInspectorMsg, NetInspectorState, PanelKind};

/// The rows for `data` under `state`: the nets the filter keeps, named nets
/// first, then by name; an expanded net with its first members, or all of
/// them once shown in full.
pub(crate) fn net_rows(data: &ProjectNetData, state: &NetInspectorState) -> Vec<NetRow> {
    let needle = state.filter.trim().to_lowercase();
    // Pin names are read only when some net is open to show them.
    let names = if state.expanded.is_empty() {
        HashMap::new()
    } else {
        pin_names(&data.index)
    };
    let mut rows: Vec<NetRow> = (0..data.index.net_count())
        .filter_map(|net| {
            let hover = data.index.hover(net, &data.classes)?;
            let matches = needle.is_empty()
                || hover.name.to_lowercase().contains(&needle)
                || hover
                    .class
                    .as_deref()
                    .is_some_and(|c| c.to_lowercase().contains(&needle));
            if !matches {
                return None;
            }
            let expanded = state.expanded.contains(&hover.name);
            let (members, more) = if expanded {
                preview(
                    members_of(data, net, &names),
                    state.full_nets.contains(&hover.name),
                )
            } else {
                (Vec::new(), 0)
            };
            Some(NetRow {
                name: hover.name,
                class: hover.class,
                pins: data.index.component_pin_count(net),
                expanded,
                members,
                more,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        let auto = |n: &str| signex_net::is_auto_name(n);
        auto(&a.name)
            .cmp(&auto(&b.name))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

/// The pins, labels, power ports and sheet entries of `net`, one per element
/// and pin, in walk order; a pin with its name from `names`. A power port
/// is one row, its port (mockup row 58).
fn members_of(
    data: &ProjectNetData,
    net: usize,
    names: &HashMap<NetAnchor, String>,
) -> Vec<NetMemberRow> {
    data.index
        .listed_members(net)
        .into_iter()
        .filter_map(|m| {
            let kind = match &m.kind {
                NetMemberKind::Pin { .. } => NetMemberRowKind::Pin,
                NetMemberKind::PowerPort => NetMemberRowKind::PowerPort,
                NetMemberKind::Label(_) => NetMemberRowKind::Label,
                NetMemberKind::SheetPin { .. } => NetMemberRowKind::SheetEntry,
                NetMemberKind::Wire | NetMemberKind::Junction => return None,
            };
            let pin = match &m.kind {
                NetMemberKind::Pin { pin, .. } => Some(pin.clone()),
                _ => None,
            };
            let anchor = NetAnchor {
                sheet: m.sheet.clone(),
                id: m.id,
                occurrence: m.occurrence.clone(),
                pin,
            };
            Some(NetMemberRow {
                kind,
                text: describe_member(m),
                pin_name: names.get(&anchor).cloned().unwrap_or_default(),
                sheet: data.sheet_label(&m.sheet),
                anchor,
            })
        })
        .collect()
}

impl Signex {
    pub(crate) fn handle_net_inspector_message(&mut self, msg: NetInspectorMsg) -> Task<Message> {
        let state = &mut self.ui_state.net_inspector;
        match msg {
            NetInspectorMsg::FilterChanged(filter) => state.filter = filter,
            NetInspectorMsg::FilterCleared => state.filter.clear(),
            NetInspectorMsg::TabSelected(tab) => state.tab = tab,
            NetInspectorMsg::NetToggled(name) => state.toggle(NetInspectorTab::ByNet, name),
            NetInspectorMsg::ComponentToggled(key) => {
                state.toggle(NetInspectorTab::ByComponent, key)
            }
            NetInspectorMsg::MoreMembersShown(name) => state.show_all(name),
            NetInspectorMsg::MemberClicked { net, anchor } => {
                return self.inspect_member(&net, &anchor);
            }
            NetInspectorMsg::OpenMessagesPressed => {
                return Task::done(Message::Overlay(OverlayMsg::OpenPanel(PanelKind::Messages)));
            }
            // A board's net rows (#2403).
            NetInspectorMsg::BoardNetSelected(net) => self.select_board_net(Some(&net)),
            NetInspectorMsg::BoardNetHighlighted(net) => self.highlight_board_net(Some(&net)),
            // A schematic net row's hover and its Highlight (mockup row 58).
            NetInspectorMsg::NetRowEntered(net) => self.hover_net_row(Some(net)),
            NetInspectorMsg::NetRowLeft(net) => {
                let held = &self.document_state.panel_ctx.net_inspector.hovered_net;
                if held.as_ref() == Some(&net) {
                    self.hover_net_row(None);
                }
            }
            NetInspectorMsg::NetHighlighted(net) => self.highlight_listed_net(&net),
        }
        Task::none()
    }

    /// The net row the pointer is over, which shows its Highlight. View
    /// state of the panel alone: the rows are not rebuilt for it.
    fn hover_net_row(&mut self, net: Option<String>) {
        self.document_state.panel_ctx.net_inspector.hovered_net = net;
    }

    /// The project the panel lists: the active document's.
    pub(super) fn inspected_project(&self) -> Option<ProjectId> {
        self.document_state
            .active_path
            .as_deref()
            .and_then(|path| self.project_of_sheet(path))
    }

    /// Where the panel's list comes from now, and the index to read it from.
    /// While a newer index is built after an edit the last one stays listed,
    /// so the list does not blink on every change.
    pub(super) fn net_inspector_source(&self) -> (NetListStatus, Option<Arc<ProjectNetData>>) {
        let Some(project) = self.inspected_project() else {
            return (NetListStatus::NoProject, None);
        };
        let Some(planned) = plan_projects(&self.document_state)
            .into_iter()
            .find(|p| p.project == project)
        else {
            return (NetListStatus::NoProject, None);
        };
        let cache = &self.document_state.net_nav.index;
        if let Some(data) = cache.fresh(project, &planned.key) {
            return (NetListStatus::Ready, Some(data));
        }
        if cache.has_failed(project, &planned.key) {
            return (NetListStatus::Unavailable, None);
        }
        match cache.latest(project) {
            Some(data) => (NetListStatus::Ready, Some(data)),
            None => (NetListStatus::Reading, None),
        }
    }

    /// What the panel's list depends on besides the index cache (whose
    /// landings clear the stored stamp): connectivity and the project list,
    /// the active document, and the filter and expanded nets.
    fn net_inspector_stamp(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.net_nav_stamp().hash(&mut hasher);
        self.document_state.active_path.hash(&mut hasher);
        self.ui_state.net_inspector.hash(&mut hasher);
        // A board in view lists its own nets, read again at each edit.
        let board = self.active_tab_path().filter(|_| self.has_active_pcb());
        let revision = board
            .as_ref()
            .and_then(|path| self.document_state.pcb_revisions.get(path).copied());
        (board, revision).hash(&mut hasher);
        hasher.finish()
    }

    /// Keep the panel's list in step with the index, and start a build of the
    /// active project's index while the panel is open and has none current.
    /// Runs from every `finish_update`; does nothing while the panel is
    /// closed, and rebuilds the rows only when their source changed.
    pub(super) fn sync_net_inspector(&mut self) -> Task<Message> {
        if !self.is_panel_open(PanelKind::NetInspector) {
            self.document_state.net_nav.inspector_source = None;
            self.document_state.net_nav.inspector_stamp = None;
            let held = &mut self.document_state.panel_ctx.net_inspector;
            if *held != NetInspectorView::default() {
                *held = NetInspectorView::default();
            }
            return Task::none();
        }
        let stamp = self.net_inspector_stamp();
        if self.document_state.net_nav.inspector_stamp == Some(stamp) {
            return Task::none();
        }
        self.document_state.net_nav.inspector_stamp = Some(stamp);
        // With a board in view the panel lists the board's nets (#2403).
        if let Some(board) = self.active_pcb() {
            let rows = crate::app::handlers::pcb_query::board_net_rows(
                board,
                &self.ui_state.net_inspector,
            );
            self.document_state.net_nav.inspector_source = None;
            self.document_state.panel_ctx.net_inspector = NetInspectorView {
                filter: self.ui_state.net_inspector.filter.clone(),
                status: NetListStatus::Ready,
                total: rows.total,
                board: Some(rows),
                ..NetInspectorView::default()
            };
            return Task::none();
        }
        let warm = self.warm_net_index_for_pin_rows();
        let (status, data) = self.net_inspector_source();
        let state = (*self.ui_state.net_inspector).clone();
        let same_source = match (&data, &self.document_state.net_nav.inspector_source) {
            (Some(data), Some((built, built_state))) => {
                Arc::ptr_eq(data, built) && *built_state == state
            }
            _ => false,
        };
        if same_source && self.document_state.panel_ctx.net_inspector.status == status {
            return warm;
        }
        // The row under the pointer stays hovered through a rebuild.
        let hovered = self
            .document_state
            .panel_ctx
            .net_inspector
            .hovered_net
            .take();
        self.document_state.panel_ctx.net_inspector = NetInspectorView {
            hovered_net: hovered,
            ..self.schematic_view(data.as_deref(), &state, status)
        };
        self.document_state.net_nav.inspector_source = data.map(|data| (data, state));
        warm
    }

    /// The panel's view of a project: only the tab shown lists its rows
    /// (a project's parts or its nets); both counts show on the tab strip.
    fn schematic_view(
        &self,
        data: Option<&ProjectNetData>,
        state: &NetInspectorState,
        status: NetListStatus,
    ) -> NetInspectorView {
        let mut view = NetInspectorView {
            tab: state.tab,
            filter: state.filter.clone(),
            status,
            ..NetInspectorView::default()
        };
        let Some(data) = data else {
            return view;
        };
        view.total = data.index.net_count();
        match state.tab {
            NetInspectorTab::ByNet => {
                view.nets = net_rows(data, state);
                view.component_total = data.index.component_count();
            }
            NetInspectorTab::ByComponent => {
                // Designators as the project's standard shows them (#1416).
                let standard = self
                    .inspected_project()
                    .and_then(|id| self.document_state.project_by_id(id))
                    .and_then(|project| project.data.designator_standard());
                (view.components, view.component_total) = component_rows(data, state, standard);
            }
        }
        view
    }

    /// A member row was clicked: highlight its net on every sheet and go to
    /// it, opening its sheet when needed.
    fn inspect_member(&mut self, net_name: &str, anchor: &NetAnchor) -> Task<Message> {
        let Some(project) = self.inspected_project() else {
            crate::diagnostics::log_info(messages::no_project("the Net Inspector"));
            return Task::none();
        };
        let Some(data) = self.ready_net_data(project) else {
            crate::diagnostics::log_info(messages::inspector_reading());
            return self.warm_net_index_for_pin_rows();
        };
        let found = data.index.resolve(net_name, anchor).and_then(|net| {
            let member = data.index.net(net)?.members.iter().find(|m| {
                m.sheet == anchor.sheet
                    && m.id == anchor.id
                    && m.occurrence == anchor.occurrence
                    && match (&m.kind, &anchor.pin) {
                        (NetMemberKind::Pin { pin, .. }, Some(want)) => pin == want,
                        (_, None) => !matches!(m.kind, NetMemberKind::Pin { .. }),
                        _ => false,
                    }
            })?;
            Some((net, member.clone()))
        });
        let Some((net, member)) = found else {
            crate::diagnostics::log_info(messages::inspector_member_gone(net_name));
            return Task::none();
        };
        let name = data
            .index
            .net(net)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        self.document_state.net_nav.highlight = Some(NetHighlight {
            project,
            name: name.clone(),
            anchor: anchor.clone(),
            cursor: None,
            elements: bright_elements(&data, net),
        });
        self.sync_net_highlight_to_canvases();
        let Some(path) = data
            .path_of(&member.sheet)
            .map(std::path::Path::to_path_buf)
        else {
            let sheet = data.sheet_label(&member.sheet);
            crate::diagnostics::log_warning(messages::net_sheet_has_no_file(&name, &sheet));
            return Task::none();
        };
        self.go_to_member(&data, &path, &member)
            .unwrap_or_else(Task::none)
    }
}
