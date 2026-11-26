//! The Object List panel (#1259): its picks, its rows kept in step with the
//! documents, and a row click, which selects through the same paths the Find
//! results use (select in place, or open the sheet and focus the object).
//!
//! Rows are read only when their source changes: the sheet in view is read
//! here, the Project scope's sheets on a blocking thread (unopened sheets come
//! from disk). Filtering, counting and the selection marks are rebuilt only
//! when the rows, the picks or the selection change.
//!
//! Each undocked document window keeps rows of its own and lists the sheet it
//! shows (#2630); names and values read as that sheet's canvas draws them.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use uuid::Uuid;

use super::super::*;
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::project_sheets::{self, OpenSheets};
use crate::app::state::WindowKind;
use crate::app::state::net_nav::ProjectNetData;
use crate::app::state::scope::CaseFold;
use crate::canvas::CanvasSlot;
use crate::panels::object_list::model::{self, ROW_PAGE, SheetLook, SheetLooks};
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
    looks: SheetLooks,
}

/// Which Object List: the one in the main slots (the main window's, or the
/// acting window's inside its swap) or an undocked window's, read in place
/// outside any swap.
#[derive(Debug, Clone, Copy)]
enum ListSlot {
    Main,
    Window(iced::window::Id),
}

/// The net a wire or bus of the sheet at `path` is on, from the cached index.
fn wire_net_of(nets: Option<&ProjectNetData>, path: &Path, id: Uuid) -> Option<String> {
    let data = nets?;
    let key = data.key_of(path)?;
    let net = *data.index.nets_of(&key, id).first()?;
    let name = &data.index.net(net)?.name;
    (!name.is_empty()).then(|| name.clone())
}

/// Whether `a` and `b` differ at most in their document facts (the clock
/// and the files' modified dates).
fn same_but_document_facts(
    a: &crate::canvas::SpecialStrings,
    b: &crate::canvas::SpecialStrings,
) -> bool {
    let mut a = a.clone();
    a.metadata.document_facts = b.metadata.document_facts.clone();
    a == *b
}

/// Whether a text of `sheet` the Object List lists could resolve a token: a
/// value or a note holding `${`, or an `=` expression.
fn lists_a_token(sheet: &signex_types::schematic::SchematicSheet) -> bool {
    let computed = |text: &str| text.contains("${") || text.trim_start().starts_with('=');
    sheet.symbols.iter().any(|s| computed(&s.value))
        || sheet.text_notes.iter().any(|t| computed(&t.text))
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
        let look = read.looks.of(&path);
        rows.extend(model::sheet_rows(sheet, &path, look, &|id: Uuid| {
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
            ObjectListMsg::CellEdit(edit) => return self.handle_object_list_cell_edit(edit),
            ObjectListMsg::ProjectRowsRead {
                window,
                generation,
                rows,
            } => {
                // A window that has closed since has no rows to land in.
                let source = match window {
                    None => Some(&mut state.source),
                    Some(window) => state.windows.get_mut(&window),
                };
                let Some(source) = source.filter(|s| s.generation == generation) else {
                    return Task::none();
                };
                source.reading = false;
                match rows {
                    Ok(rows) => {
                        source.rows = rows;
                        source.failed = false;
                    }
                    Err(why) => {
                        // The stamp stays: a read that fails every time is
                        // tried again when its source changes, not on every
                        // update.
                        source.failed = true;
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

    /// Whether a listed text of an open sheet `scope` reads from `active`
    /// could resolve a token, so the clock or a modified date may change
    /// it. A sheet with no open document is read with no special strings.
    fn object_texts_use_tokens(&self, active: &Path, scope: ObjectScope) -> bool {
        let engines = &self.document_state.engines;
        match scope {
            ObjectScope::ThisSheet => engines
                .get(active)
                .is_some_and(|engine| lists_a_token(engine.document())),
            ObjectScope::Project => engines
                .values()
                .any(|engine| lists_a_token(engine.document())),
        }
    }

    /// The cached net index of the project that owns `path`, if one is built.
    fn object_list_nets(&self, path: &Path) -> Option<Arc<ProjectNetData>> {
        let project = self.document_state.document_project(path)?.id;
        self.document_state.net_nav.index.latest(project)
    }

    /// The window whose rows `slot` lists; `None` for the main window's.
    fn list_owner(&self, slot: ListSlot) -> Option<iced::window::Id> {
        match slot {
            ListSlot::Main => self.interaction_state.acting_window,
            ListSlot::Window(window) => Some(window),
        }
    }

    /// The canvas the sheet of `slot` is drawn on.
    fn list_canvas(&self, slot: ListSlot) -> Option<&CanvasSlot> {
        match slot {
            ListSlot::Main => Some(self.interaction_state.active_canvas()),
            ListSlot::Window(window) => self.interaction_state.canvases.get(&window),
        }
    }

    /// The schematic `slot` lists; `None` for a board or no document.
    fn list_sheet(&self, slot: ListSlot) -> Option<PathBuf> {
        match slot {
            ListSlot::Main => self
                .document_state
                .active_path
                .clone()
                .filter(|_| self.has_active_schematic() && !self.has_active_pcb()),
            ListSlot::Window(window) => match self.ui_state.windows.get(&window) {
                Some(WindowKind::UndockedTab { path, .. })
                    if self.document_state.engines.contains_key(path) =>
                {
                    Some(path.clone())
                }
                _ => None,
            },
        }
    }

    /// The panel view `slot` draws.
    fn list_view_mut(&mut self, slot: ListSlot) -> Option<&mut ObjectListView> {
        match slot {
            ListSlot::Main => Some(&mut self.document_state.panel_ctx.object_list),
            ListSlot::Window(window) => self
                .document_state
                .window_panel_ctx
                .get_mut(&window)
                .map(|ctx| &mut ctx.object_list),
        }
    }

    /// How the canvas draws the sheet at `path`, with `special` as what its
    /// `${TOKEN}`s resolve to.
    pub(crate) fn sheet_look(
        &self,
        path: &Path,
        special: Option<crate::canvas::SpecialStrings>,
    ) -> SheetLook {
        SheetLook {
            placement: self.shown_placement(path),
            special,
            standard: self.document_state.designator_standard_for(path),
        }
    }

    /// The looks of a Project read of `active`'s project: every page, every
    /// sheet its hierarchy places and `active` itself, whose `${TOKEN}`s are
    /// its canvas's (`active_special`).
    fn project_looks(
        &self,
        active: &Path,
        pages: &[PathBuf],
        active_special: Option<crate::canvas::SpecialStrings>,
    ) -> SheetLooks {
        let mut paths: Vec<PathBuf> = pages.to_vec();
        if let Some((hierarchy, _)) = self.hierarchy_describing(active) {
            paths.extend(hierarchy.node_paths().into_iter().map(Path::to_path_buf));
        }
        let mut by_path = std::collections::HashMap::new();
        for path in paths {
            if path == active || by_path.contains_key(&path) {
                continue;
            }
            let special = self.special_strings_for_path(&path);
            let look = self.sheet_look(&path, special);
            by_path.insert(path, look);
        }
        let look = self.sheet_look(active, active_special);
        by_path.insert(active.to_path_buf(), look);
        SheetLooks {
            by_path,
            fallback: SheetLook {
                standard: self.document_state.designator_standard_for(active),
                ..SheetLook::default()
            },
        }
    }

    /// Stamp of what the rows of `scope` are read from, listed for the sheet
    /// at `active`.
    fn object_rows_stamp(&self, active: &Path, scope: ObjectScope) -> u64 {
        let ds = &self.document_state;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        scope.hash(&mut h);
        active.hash(&mut h);
        let nets = self.object_list_nets(active);
        nets.as_ref().map(|n| Arc::as_ptr(n) as usize).hash(&mut h);
        // What the names and values are drawn with (#2630).
        self.shown_placement(active).hash(&mut h);
        ds.designator_standard_for(active).hash(&mut h);
        let project = ds.document_project(active);
        project.map(|p| &p.data.parameters).hash(&mut h);
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
                project_sheets::project_pages(project).hash(&mut h);
                // The other sheets' placements come with their hierarchy.
                let hierarchy = self.hierarchy_describing(active);
                let placed = hierarchy.map(|(tree, settled)| (tree.node_paths(), settled));
                placed.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Keep every Object List's rows in step with the documents, and its view
    /// with the rows, the picks and its canvas's selection: the main slots'
    /// list, then, outside a window's swap, each undocked window's. Runs from
    /// every `finish_update`; does nothing while the panel is closed or
    /// nothing a list reads changed.
    pub(crate) fn sync_object_list(&mut self) -> Task<Message> {
        let main = self.sync_object_list_in(ListSlot::Main);
        // Inside a window's swap the main slots hold that window's list.
        if self.interaction_state.acting_window.is_some() {
            return main;
        }
        let ui = &mut self.ui_state;
        ui.object_list
            .windows
            .retain(|window, _| ui.windows.contains_key(window));
        let windows: Vec<iced::window::Id> = self
            .document_state
            .window_panel_ctx
            .keys()
            .copied()
            .filter(|window| self.is_document_window(*window))
            .collect();
        let mut tasks = vec![main];
        for window in windows {
            tasks.push(self.sync_object_list_in(ListSlot::Window(window)));
        }
        Task::batch(tasks)
    }

    /// [`Self::sync_object_list`] for the list of `slot`.
    fn sync_object_list_in(&mut self, slot: ListSlot) -> Task<Message> {
        let owner = self.list_owner(slot);
        if !self.is_panel_open(PanelKind::ObjectList) {
            let source = self.ui_state.object_list.source_of_mut(owner);
            if source.stamp.is_some() {
                source.reset();
                if let Some(view) = self.list_view_mut(slot) {
                    *view = ObjectListView::default();
                }
            }
            return Task::none();
        }
        let Some(active) = self.list_sheet(slot) else {
            let source = self.ui_state.object_list.source_of_mut(owner);
            if source.stamp.is_some() {
                source.reset();
            }
            if let Some(view) = self.list_view_mut(slot)
                && view.status != ObjectListStatus::NoSchematic
            {
                *view = ObjectListView::default();
            }
            return Task::none();
        };
        let task = self.refresh_object_rows(slot, &active);
        self.refresh_object_list_view(slot, active);
        task
    }

    /// Read the rows of `slot` again when their source changed: at once for
    /// the sheet in view, on a blocking thread for the project.
    fn refresh_object_rows(&mut self, slot: ListSlot, active: &Path) -> Task<Message> {
        let owner = self.list_owner(slot);
        let ds = &self.document_state;
        // A loose sheet has no project: Project lists the sheet itself.
        let root = ds
            .document_project(active)
            .map(|_| project_sheets::resolve_netlist_root_for(ds, active));
        let scope = match root {
            Some(_) => self.ui_state.object_list.scope,
            None => ObjectScope::ThisSheet,
        };
        let stamp = self.object_rows_stamp(active, scope);
        let canvas_special = self
            .list_canvas(slot)
            .and_then(|canvas| canvas.special_strings.as_ref());
        let source = self.ui_state.object_list.source_of(owner);
        let read_from = source.filter(|s| s.stamp == Some(stamp));
        if read_from.is_some_and(|s| s.special.as_ref() == canvas_special) {
            return Task::none();
        }
        let special = canvas_special.cloned();
        // The clock ticks and every save moves a modified date: rows no
        // listed text of which uses a token read the same (#2630).
        // A first read before the canvas had its strings reads the same too.
        if let Some(read) = read_from
            && match (read.special.as_ref(), special.as_ref()) {
                (Some(read), Some(now)) => same_but_document_facts(read, now),
                (None, _) => true,
                (Some(_), None) => false,
            }
            && !self.object_texts_use_tokens(active, scope)
        {
            self.ui_state.object_list.source_of_mut(owner).special = special;
            return Task::none();
        }
        let nets = self.object_list_nets(active);
        // Wire rows name their net from the project index (#3039): build one
        // when none is current; its landing moves the stamp and rereads.
        let warm = self.warm_net_index_of(active);
        if scope == ObjectScope::ThisSheet {
            let look = self.sheet_look(active, special.clone());
            let rows = match self.document_state.engines.get(active) {
                Some(engine) => model::sheet_rows(engine.document(), active, &look, &|id: Uuid| {
                    wire_net_of(nets.as_deref(), active, id)
                }),
                None => Vec::new(),
            };
            let source = self.ui_state.object_list.source_of_mut(owner);
            source.stamp = Some(stamp);
            source.special = special;
            source.generation += 1;
            source.rows = Arc::new(rows);
            source.reading = false;
            source.failed = false;
            return warm;
        }
        let Some(root) = root else {
            return warm;
        };
        let ds = &self.document_state;
        let project = ds.document_project(active);
        let pages = project_sheets::project_pages(project);
        let read = ProjectRead {
            open: project_sheets::open_sheets_snapshot(ds),
            looks: self.project_looks(active, &pages, special.clone()),
            pages,
            root,
            fold: crate::app::state::project_case::fold_of(project),
            active: Some(active.to_path_buf()),
            nets,
        };
        let source = self.ui_state.object_list.source_of_mut(owner);
        source.stamp = Some(stamp);
        source.special = special;
        source.generation += 1;
        source.reading = true;
        let generation = source.generation;
        let read = Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_project_rows(read))
                    .await
                    .map(Arc::new)
                    .map_err(|e| format!("Object List: the project read failed: {e}"))
            },
            move |rows| {
                Message::Dock(crate::dock::DockMessage::Panel(PanelMsg::ObjectList(
                    ObjectListMsg::ProjectRowsRead {
                        window: owner,
                        generation,
                        rows,
                    },
                )))
            },
        );
        Task::batch([warm, read])
    }

    /// Build the view of `slot` again when its rows, the picks, the sheet in
    /// view or its canvas's selection changed.
    fn refresh_object_list_view(&mut self, slot: ListSlot, active: PathBuf) {
        let owner = self.list_owner(slot);
        let state = &self.ui_state.object_list;
        let (Some(canvas), Some(source)) = (self.list_canvas(slot), state.source_of(owner)) else {
            return;
        };
        let selected = &canvas.selected;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        state.picks_stamp().hash(&mut h);
        (Arc::as_ptr(&source.rows) as usize, source.reading).hash(&mut h);
        active.hash(&mut h);
        // The filter folds by the language's rules.
        self.ui_state.i18n.locale().hash(&mut h);
        selected.hash(&mut h);
        let stamp = h.finish();
        if source.view_stamp == Some(stamp) {
            return;
        }
        let status = if source.reading && source.rows.is_empty() {
            ObjectListStatus::Reading
        } else {
            ObjectListStatus::Ready
        };
        let i18n = &self.ui_state.i18n;
        let view = ObjectListView::build(state, source, status, Some(active), selected, i18n);
        self.ui_state.object_list.source_of_mut(owner).view_stamp = Some(stamp);
        if let Some(slot_view) = self.list_view_mut(slot) {
            *slot_view = view;
        }
    }
}

#[cfg(test)]
#[path = "object_list_tests.rs"]
mod tests;
