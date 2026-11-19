//! The Object List panel (#1259): its picks, its rows kept in step with the
//! documents, and a row click, which selects through the same paths the Find
//! results use (select in place, or open the sheet and focus the object).
//!
//! Rows are read only when their source changes: the sheet in view is read
//! here, the Project scope's sheets on a blocking thread (unopened sheets come
//! from disk). Filtering, counting and the selection marks are rebuilt only
//! when the rows, the picks or the selection change.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use uuid::Uuid;

use super::super::*;
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::net_nav::ProjectNetData;
use crate::app::state::scope::CaseFold;
use crate::panels::object_list::model::{self, ROW_PAGE};
use crate::panels::{
    KindChip, ObjectListMsg, ObjectListStatus, ObjectListView, ObjectRef, ObjectRow, ObjectScope,
    PanelKind, PanelMsg,
};

/// What a Project scope read needs, captured from `update` (memory only).
struct ProjectRead {
    open: OpenSheets,
    pages: Vec<PathBuf>,
    root: PathBuf,
    fold: CaseFold,
    active: Option<PathBuf>,
    nets: Option<Arc<ProjectNetData>>,
}

/// The net a wire or bus of the sheet at `path` is on, from the cached index.
fn wire_net_of(nets: Option<&ProjectNetData>, path: &Path, id: Uuid) -> Option<String> {
    let data = nets?;
    let key = data.key_of(path)?;
    let net = *data.index.nets_of(&key, id).first()?;
    let name = &data.index.net(net)?.name;
    (!name.is_empty()).then(|| name.clone())
}

/// Every project sheet's rows, the sheet in view first and the rest by path.
fn read_project_rows(read: ProjectRead) -> Vec<ObjectRow> {
    let set = project_sheets::assemble_project_sheets_quiet_from(
        &read.open,
        &read.pages,
        &read.root,
        read.fold,
    );
    let mut paths = project_sheets::ordered_project_sheet_paths(&set, read.active.as_deref());
    if let Some(active) = read.active.as_ref()
        && let Some(at) = paths.iter().position(|p| p == active)
    {
        let first = paths.remove(at);
        paths.insert(0, first);
    }
    let nets = read.nets.as_deref();
    let mut rows = Vec::new();
    for path in paths {
        let Some(sheet) = set.sheets.get(&path).or_else(|| read.open.get(&path)) else {
            continue;
        };
        rows.extend(model::sheet_rows(sheet, &path, &|id: Uuid| {
            wire_net_of(nets, &path, id)
        }));
    }
    rows
}

impl Signex {
    pub(crate) fn handle_object_list_message(&mut self, msg: ObjectListMsg) -> Task<Message> {
        let state = &mut self.ui_state.object_list;
        match msg {
            ObjectListMsg::FilterChanged(filter) => {
                state.filter = filter;
                state.shown = ROW_PAGE;
            }
            ObjectListMsg::ScopePicked(scope) => {
                state.scope = scope;
                state.shown = ROW_PAGE;
            }
            ObjectListMsg::KindChipToggled(KindChip::All) => state.kinds.clear(),
            ObjectListMsg::KindChipToggled(KindChip::Kind(kind)) => {
                state.kinds.toggle(kind);
            }
            ObjectListMsg::MoreToggled => state.more_open = !state.more_open,
            ObjectListMsg::MoreRowsShown => state.shown = state.row_limit() + ROW_PAGE,
            ObjectListMsg::RowClicked(target) => return self.go_to_object(&target, false),
            ObjectListMsg::RowDoubleClicked(target) => return self.go_to_object(&target, true),
            ObjectListMsg::ProjectRowsRead { generation, rows } => {
                if generation != state.source.generation {
                    return Task::none();
                }
                state.source.reading = false;
                match rows {
                    Ok(rows) => state.source.rows = rows,
                    Err(why) => {
                        // Read again at the next change rather than never.
                        state.source.stamp = None;
                        crate::diagnostics::log_warning(why);
                    }
                }
            }
        }
        Task::none()
    }

    /// Select `target` on the canvas; `zoom` also frames it. An object on
    /// another sheet opens that sheet and lands on it once it shows.
    fn go_to_object(&mut self, target: &ObjectRef, zoom: bool) -> Task<Message> {
        let held = self.resolve_held_spelling(target.sheet.clone());
        if self.document_state.active_path.as_deref() == Some(held.as_path()) {
            if zoom {
                return self.handle_focus_at(target.at.x, target.at.y, Some(target.item));
            }
            self.replace_selection(vec![target.item]);
            return Task::none();
        }
        let placement = self
            .document_state
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .and_then(|h| crate::app::state::default_placement(h.placements_of(&held)));
        let then = AfterOpen::FocusAt {
            x: target.at.x,
            y: target.at.y,
            item: Some(target.item),
        };
        let open = SheetOpen::at(placement).then(then);
        // A window cannot switch sheets inside its swap: the open is queued
        // to the main window, as Find's is.
        if self.interaction_state.acting_window.is_some() {
            return Task::done(Message::Project(ProjectMsg::SheetOpenRequested {
                path: target.sheet.clone(),
                open: Box::new(open),
            }));
        }
        self.open_sheet(target.sheet.clone(), open)
    }

    /// The cached net index of the project that owns `path`, if one is built.
    fn object_list_nets(&self, path: &Path) -> Option<Arc<ProjectNetData>> {
        let project = self.document_state.document_project(path)?.id;
        self.document_state.net_nav.index.latest(project)
    }

    /// Stamp of what the rows of `scope` are read from.
    fn object_rows_stamp(&self, active: &Path, scope: ObjectScope) -> u64 {
        let ds = &self.document_state;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        scope.hash(&mut h);
        active.hash(&mut h);
        let nets = self.object_list_nets(active);
        nets.as_ref().map(|n| Arc::as_ptr(n) as usize).hash(&mut h);
        let engine_stamp = |path: &Path, h: &mut std::collections::hash_map::DefaultHasher| {
            if let Some(engine) = ds.engines.get(path) {
                (path, engine.instance_id(), engine.revision()).hash(h);
            }
        };
        match scope {
            ObjectScope::ThisSheet => engine_stamp(active, &mut h),
            ObjectScope::Project => {
                let mut open: Vec<&PathBuf> = ds.engines.keys().collect();
                open.sort();
                for path in open {
                    engine_stamp(path, &mut h);
                }
                project_sheets::declared_pages(ds).hash(&mut h);
            }
        }
        h.finish()
    }

    /// Keep the panel's rows in step with the documents, and its view with
    /// the rows, the picks and the canvas selection. Runs from every
    /// `finish_update`; does nothing while the panel is closed or nothing it
    /// reads changed.
    pub(crate) fn sync_object_list(&mut self) -> Task<Message> {
        if !self.is_panel_open(PanelKind::ObjectList) {
            let source = &mut self.ui_state.object_list.source;
            if source.stamp.is_some() {
                *source = Default::default();
                self.document_state.panel_ctx.object_list = ObjectListView::default();
            }
            return Task::none();
        }
        let active = self
            .document_state
            .active_path
            .clone()
            .filter(|_| self.has_active_schematic() && !self.has_active_pcb());
        let Some(active) = active else {
            self.ui_state.object_list.source.stamp = None;
            self.ui_state.object_list.source.view_stamp = None;
            if self.document_state.panel_ctx.object_list.status != ObjectListStatus::NoSchematic {
                self.document_state.panel_ctx.object_list = ObjectListView::default();
            }
            return Task::none();
        };
        let task = self.refresh_object_rows(&active);
        self.refresh_object_list_view(active);
        task
    }

    /// Read the rows again when their source changed: at once for the sheet
    /// in view, on a blocking thread for the project.
    fn refresh_object_rows(&mut self, active: &Path) -> Task<Message> {
        // A loose sheet has no project: Project lists the sheet itself.
        let root = self
            .document_state
            .active_document_project()
            .and_then(|_| project_sheets::resolve_netlist_root(&self.document_state));
        let scope = match root {
            Some(_) => self.ui_state.object_list.scope,
            None => ObjectScope::ThisSheet,
        };
        let stamp = self.object_rows_stamp(active, scope);
        if self.ui_state.object_list.source.stamp == Some(stamp) {
            return Task::none();
        }
        self.ui_state.object_list.source.stamp = Some(stamp);
        let nets = self.object_list_nets(active);
        if scope == ObjectScope::ThisSheet {
            let rows = match self.document_state.active_engine() {
                Some(engine) => model::sheet_rows(engine.document(), active, &|id: Uuid| {
                    wire_net_of(nets.as_deref(), active, id)
                }),
                None => Vec::new(),
            };
            let source = &mut self.ui_state.object_list.source;
            source.generation += 1;
            source.rows = Arc::new(rows);
            source.reading = false;
            return Task::none();
        }
        let Some(root) = root else {
            return Task::none();
        };
        let ds = &self.document_state;
        let read = ProjectRead {
            open: project_sheets::open_sheets_snapshot(ds),
            pages: project_sheets::declared_pages(ds),
            root,
            fold: crate::app::state::project_case::fold_of(ds.active_document_project()),
            active: Some(active.to_path_buf()),
            nets,
        };
        let source = &mut self.ui_state.object_list.source;
        source.generation += 1;
        source.reading = true;
        let generation = source.generation;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_project_rows(read))
                    .await
                    .map(Arc::new)
                    .map_err(|e| format!("Object List: the project read failed: {e}"))
            },
            move |rows| {
                Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::ObjectList(
                    ObjectListMsg::ProjectRowsRead { generation, rows },
                )))
            },
        )
    }

    /// Build the panel's view again when the rows, the picks, the sheet in
    /// view or the canvas selection changed.
    fn refresh_object_list_view(&mut self, active: PathBuf) {
        let state = &self.ui_state.object_list;
        let selected = &self.interaction_state.active_canvas().selected;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        state.picks_stamp().hash(&mut h);
        (
            Arc::as_ptr(&state.source.rows) as usize,
            state.source.reading,
        )
            .hash(&mut h);
        active.hash(&mut h);
        selected.hash(&mut h);
        let stamp = h.finish();
        if state.source.view_stamp == Some(stamp) {
            return;
        }
        let status = if state.source.reading && state.source.rows.is_empty() {
            ObjectListStatus::Reading
        } else {
            ObjectListStatus::Ready
        };
        let view = ObjectListView::build(state, status, Some(active), selected);
        self.ui_state.object_list.source.view_stamp = Some(stamp);
        self.document_state.panel_ctx.object_list = view;
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::{Point, SchematicSheet, Symbol};

    use super::*;

    fn sheet(n: u128, reference: &str) -> SchematicSheet {
        let mut s = SchematicSheet::default();
        s.symbols.push(Symbol {
            uuid: Uuid::from_u128(n),
            reference: reference.into(),
            value: "10k".into(),
            position: Point::new(1.0, 1.0),
            ..Symbol::default()
        });
        s
    }

    /// #1259: Project lists every page of the project, the sheet in view
    /// first; This sheet lists only the sheet in view.
    #[test]
    fn the_project_scope_lists_every_sheet_with_the_sheet_in_view_first() {
        let dir = tempfile::tempdir().expect("temp dir");
        let (a, b) = (dir.path().join("a.snxsch"), dir.path().join("b.snxsch"));
        for (path, s) in [(&a, sheet(1, "R1")), (&b, sheet(2, "R2"))] {
            let text = signex_types::format::SnxSchematic::new(s)
                .write_string()
                .expect("encode");
            std::fs::write(path, text).expect("write");
        }
        let open: OpenSheets = [(b.clone(), sheet(2, "R2"))].into_iter().collect();
        let rows = read_project_rows(ProjectRead {
            open,
            pages: vec![a.clone(), b.clone()],
            root: a.clone(),
            fold: CaseFold::host_default(),
            active: Some(b.clone()),
            nets: None,
        });
        let sheets: Vec<&str> = rows.iter().map(|r| r.sheet.as_str()).collect();
        assert_eq!(sheets, ["b", "a"], "the sheet in view first: {rows:?}");

        let this_sheet = model::sheet_rows(&sheet(2, "R2"), &b, &|_: Uuid| None);
        assert_eq!(this_sheet.len(), 1);
        assert_eq!(this_sheet[0].sheet, "b");
    }
}
