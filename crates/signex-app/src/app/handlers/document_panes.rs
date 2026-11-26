//! Document panes in the main window (mockup row 207, #1307): a tab dropped
//! on an edge of the document area, View ▸ Window ▸ Split Right / Split Down
//! / Open Second View, and the split kept per project. The rules live in
//! `crate::document_panes`; this file turns them into Model changes and the
//! `prefs.json` write. The panes draw through the tiled main window
//! (`handlers::document_tiling`, `view::tiled_panes`). Design:
//! docs/MENU_BAR.md, "View ▸ Window ▸ Split".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_i18n::{MessageId, ids, t};

use crate::app::{Message, Signex, TabInfo, TabKind, TileLayout, WindowMsg};
use crate::diagnostics::{log_error_line, log_warning};
use crate::document_panes::{
    DropEdge, DropTarget, MAX_PANES, PaneSplit, PaneTab, SplitRefusal, drop_target, store,
};

/// The main window's panes as it draws them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TiledPanes {
    pub layout: TileLayout,
    /// The tab index each pane draws, in pane order; a second view repeats
    /// its tab.
    pub tabs: Vec<usize>,
    /// The pane with the focus, which draws the active tab.
    pub focused: usize,
}

/// The pane key of `tab`: its file and which page of it the tab shows.
pub(crate) fn pane_tab(tab: &TabInfo) -> PaneTab {
    let page = match &tab.kind {
        TabKind::Schematic => "schematic",
        TabKind::Pcb => "pcb",
        TabKind::GerberViewer => "gerber",
        TabKind::ComponentEditor(_) => "component",
        TabKind::SymbolEditor(_) => "symbol",
        TabKind::FootprintEditor(_) => "footprint",
        TabKind::LibraryBrowser(_) => "library",
        TabKind::Project(_) => "project",
        TabKind::Rules(_) => "rules",
        TabKind::PolygonManager => "polygons",
        TabKind::LayerStack => "layer_stack",
        TabKind::VariantManager(_) => "variants",
        TabKind::PinTable => "pin_table",
    };
    PaneTab {
        path: tab.path.clone(),
        page: page.to_string(),
    }
}

/// The catalog line that says why a split was not made.
pub(crate) fn refusal_line(why: SplitRefusal) -> MessageId {
    match why {
        SplitRefusal::NotDocked => ids::WORKSPACE_LAYOUTS_PANES_NOT_DOCKED,
        SplitRefusal::LastTab => ids::WORKSPACE_LAYOUTS_PANES_LAST_TAB,
        SplitRefusal::Crosswise => ids::WORKSPACE_LAYOUTS_PANES_CROSSWISE,
        SplitRefusal::TooManyPanes => ids::WORKSPACE_LAYOUTS_PANES_TOO_MANY,
    }
}

impl Signex {
    /// The main window's tabs (indices in strip order) and their pane keys.
    fn docked_pane_tabs(&self) -> (Vec<usize>, Vec<PaneTab>) {
        let tabs = &self.document_state.tabs;
        self.docked_tab_indices()
            .into_iter()
            .filter_map(|index| tabs.get(index).map(|tab| (index, pane_tab(tab))))
            .unzip()
    }

    fn active_pane_tab(&self) -> Option<PaneTab> {
        let active = self.document_state.active_tab;
        self.document_state.tabs.get(active).map(pane_tab)
    }

    /// The `.snxprj` of the active tab's project; a loose file has none.
    fn active_project_file(&self) -> Option<PathBuf> {
        let tab = self
            .document_state
            .tabs
            .get(self.document_state.active_tab)?;
        let project = self.document_state.project_by_id(tab.project_id?)?;
        Some(project.path.clone())
    }

    /// The panes the main window draws: one per docked tab under View ▸
    /// Window ▸ Tile, the split's panes after a drop. `None` unless two
    /// panes or more show, the active tab among them.
    pub(crate) fn tiled_pane_view(&self) -> Option<TiledPanes> {
        let layout = self.ui_state.document_tiling?;
        let active = self.document_state.active_tab;
        let Some(split) = self.ui_state.workspace_layouts.panes.split.as_ref() else {
            let tabs = self.docked_tab_indices();
            let focused = tabs.iter().position(|&tab| tab == active)?;
            return (tabs.len() >= 2).then_some(TiledPanes {
                layout,
                tabs,
                focused,
            });
        };
        let (docked, keys) = self.docked_pane_tabs();
        let at = docked.iter().position(|&tab| tab == active)?;
        let shown = split.resolve(&keys);
        if shown.len() < 2 {
            return None;
        }
        let focused = split.focused_of(&shown, at)?;
        let tabs = split
            .drawn_tabs(&keys, &shown, focused, at)
            .into_iter()
            .map(|position| docked[position])
            .collect();
        Some(TiledPanes {
            layout: split.direction,
            tabs,
            focused,
        })
    }

    /// The tab a document-tab drag carries past the strip's drag threshold
    /// and the edge a release now would split on. `None` over the middle,
    /// off the document area, or on an edge the split refuses (across its
    /// panes, or past four).
    pub(crate) fn pane_drop_target(&self) -> Option<(usize, DropEdge)> {
        let (tab, ox, oy) = self.ui_state.tab_dragging?;
        let (mx, my) = self.interaction_state.last_mouse_pos;
        let threshold = crate::app::view::TAB_DRAG_THRESHOLD_PX;
        if (mx - ox).powi(2) + (my - oy).powi(2) <= threshold * threshold {
            return None;
        }
        if !self.docked_tab_indices().contains(&tab) {
            return None;
        }
        let area = self.document_area_in(None);
        let target = drop_target((area.x, area.y, area.width, area.height), (mx, my))?;
        let DropTarget::Edge(edge) = target else {
            return None;
        };
        let refused = self
            .ui_state
            .workspace_layouts
            .panes
            .split
            .as_ref()
            .is_some_and(|split| {
                split.direction != edge.direction() || split.panes.len() >= MAX_PANES
            });
        (!refused).then_some((tab, edge))
    }

    /// Split tab `tab` into a new pane on `edge`: moved there, or with
    /// `second_view` shown there as well. The tab becomes active in the new
    /// pane. View ▸ Window ▸ Tile's one pane per document gives way to the
    /// split.
    pub(crate) fn split_document_pane(
        &mut self,
        tab: usize,
        edge: DropEdge,
        second_view: bool,
    ) -> Result<(), SplitRefusal> {
        let (docked, keys) = self.docked_pane_tabs();
        if !docked.contains(&tab) {
            return Err(SplitRefusal::NotDocked);
        }
        let before = self.canvas_bounds();
        let key = self
            .document_state
            .tabs
            .get(tab)
            .map(pane_tab)
            .ok_or(SplitRefusal::NotDocked)?;
        let active = self.active_pane_tab().unwrap_or_else(|| key.clone());
        let panes = &mut self.ui_state.workspace_layouts.panes;
        let split = match panes.split.take() {
            Some(mut split) => {
                let added = split.add(&keys, &key, edge, second_view);
                panes.split = Some(split);
                added?;
                panes.split.take()
            }
            None => Some(PaneSplit::first(&keys, &key, &active, edge, second_view)?),
        };
        let direction = split.as_ref().map_or(edge.direction(), |s| s.direction);
        self.ui_state.workspace_layouts.panes.split = split;
        self.ui_state.document_tiling = Some(direction);
        if tab == self.document_state.active_tab {
            self.settle_document_tiling();
        } else {
            self.park_active_schematic_session();
            self.document_state.active_tab = tab;
            self.sync_active_tab();
        }
        self.follow_main_canvas_resize(before);
        Ok(())
    }

    /// View ▸ Window ▸ Split Right / Split Down / Open Second View: the
    /// active tab split on `edge`, kept for its project; a refusal is said
    /// in Messages.
    pub(crate) fn split_active_document_pane(
        &mut self,
        edge: DropEdge,
        second_view: bool,
    ) -> Task<Message> {
        let tab = self.document_state.active_tab;
        self.split_and_keep(tab, edge, second_view)
            .unwrap_or_else(|why| self.say_split_refused(why))
    }

    /// [`Self::split_document_pane`], then the split kept for its project.
    pub(crate) fn split_and_keep(
        &mut self,
        tab: usize,
        edge: DropEdge,
        second_view: bool,
    ) -> Result<Task<Message>, SplitRefusal> {
        self.split_document_pane(tab, edge, second_view)?;
        Ok(self.keep_document_panes())
    }

    fn say_split_refused(&mut self, why: SplitRefusal) -> Task<Message> {
        let line = t!(self.ui_state.i18n, refusal_line(why));
        log_warning(line.into_string());
        self.sync_diagnostics_panel_ctx();
        Task::none()
    }

    /// A document tab let go over the main window (`released_in`): on an
    /// edge of the document area it splits there, as a second view while
    /// Ctrl is held. Read before the drag state is cleared;
    /// [`Self::drop_tab_on_pane_edge`] makes the split after.
    pub(crate) fn pane_drop_on_release(
        &self,
        released_in: iced::window::Id,
    ) -> Option<(usize, DropEdge, bool)> {
        if self.ui_state.main_window_id != Some(released_in) {
            return None;
        }
        let (tab, edge) = self.pane_drop_target()?;
        let second_view = self.ui_state.workspace_layouts.panes.second_view_held;
        Some((tab, edge, second_view))
    }

    /// Make the split a release asked for (`pane_drop_on_release`).
    pub(crate) fn drop_tab_on_pane_edge(
        &mut self,
        (tab, edge, second_view): (usize, DropEdge, bool),
    ) -> Task<Message> {
        self.split_and_keep(tab, edge, second_view)
            .unwrap_or_else(|why| self.say_split_refused(why))
    }

    /// Before the tiling settles (a tab switch, close, undock or open): the
    /// split forgets the tabs that left, follows the active tab and ends
    /// under two panes; with no tiling, the split kept for the active tab's
    /// project may come back. Whether a split shows now.
    pub(crate) fn settle_document_panes(&mut self) -> bool {
        let (_, keys) = self.docked_pane_tabs();
        let active = self.active_pane_tab();
        let panes = &mut self.ui_state.workspace_layouts.panes;
        if let Some(split) = panes.split.as_mut() {
            if split.prune(&keys) {
                if let Some(active) = &active {
                    split.follow_active(&keys, active);
                }
                return true;
            }
            // Closing down to one pane ends the split; what is kept for
            // the project stays, so a reopened project gets it back.
            panes.split = None;
            self.ui_state.document_tiling = None;
            return false;
        }
        if self.ui_state.document_tiling.is_none() {
            self.restore_kept_panes(&keys, active.as_ref());
        }
        self.ui_state.workspace_layouts.panes.split.is_some()
    }

    /// The split kept for the active tab's project, brought back once per
    /// session as soon as two of its panes have a document open.
    fn restore_kept_panes(&mut self, keys: &[PaneTab], active: Option<&PaneTab>) {
        let Some(project) = self.active_project_file() else {
            return;
        };
        let panes = &self.ui_state.workspace_layouts.panes;
        if panes.restored.contains(&project) {
            return;
        }
        let Some(entry) = panes.kept.get(&project_key(&project)) else {
            return;
        };
        let Some(mut split) = store::split_from_json(entry, project_folder(&project)) else {
            return;
        };
        if !split.prune(keys) {
            return;
        }
        if let Some(active) = active {
            split.follow_active(keys, active);
        }
        let before = self.canvas_bounds();
        self.ui_state.document_tiling = Some(split.direction);
        let panes = &mut self.ui_state.workspace_layouts.panes;
        panes.restored.insert(project);
        panes.split = Some(split);
        self.follow_main_canvas_resize(before);
    }

    /// Keep the split for the active tab's project, or forget the kept one
    /// when the split is gone, written off the UI thread. A loose file has
    /// no project and keeps nothing; forgetting what was never kept writes
    /// nothing.
    pub(crate) fn keep_document_panes(&mut self) -> Task<Message> {
        let Some(project) = self.active_project_file() else {
            return Task::none();
        };
        let key = project_key(&project);
        let folder = project_folder(&project).to_path_buf();
        let panes = &mut self.ui_state.workspace_layouts.panes;
        panes.restored.insert(project);
        match &panes.split {
            Some(split) => {
                let entry = store::split_to_json(split, &folder, panes.kept.get(&key));
                panes.kept.insert(key, entry);
            }
            None => {
                if panes.kept.remove(&key).is_none() {
                    return Task::none();
                }
            }
        }
        let value = store::kept_to_json(&panes.kept);
        let sink = Arc::clone(&panes.store);
        crate::fonts::background_pref_merge(
            store::PREFS_KEY,
            move || sink.write(&value),
            |result| Message::Window(WindowMsg::DocumentPanesKept(result)),
        )
    }

    /// The kept panes' write finished; a failure is said in Messages.
    pub(crate) fn document_panes_kept(&mut self, result: Result<(), String>) -> Task<Message> {
        if let Err(why) = result {
            let line = t!(
                self.ui_state.i18n,
                ids::WORKSPACE_LAYOUTS_PANES_NOT_KEPT,
                "why" => why
            );
            log_error_line(line.into_string());
            self.sync_diagnostics_panel_ctx();
        }
        Task::none()
    }

    /// The split ends by the user's hand (Untile, or Tile's one pane per
    /// document): the main window shows one document until the caller
    /// tiles again, and the project forgets the split. `Task::none()` when
    /// there was no split.
    pub(crate) fn forget_document_split(&mut self) -> Task<Message> {
        if self.ui_state.workspace_layouts.panes.split.take().is_none() {
            return Task::none();
        }
        self.ui_state.document_tiling = None;
        self.keep_document_panes()
    }

    /// A press on shown pane `pane`: it takes the focus. With a split this
    /// is the pane, not the tab, so the two views of one document each
    /// take it. The tab it draws, to make active.
    pub(crate) fn focus_split_pane(&mut self, pane: usize) -> Option<usize> {
        let (docked, keys) = self.docked_pane_tabs();
        let active = self.document_state.active_tab;
        let at = docked.iter().position(|&tab| tab == active)?;
        let split = self.ui_state.workspace_layouts.panes.split.as_mut()?;
        let target = split.focus(&keys, pane, at)?;
        docked.get(target).copied()
    }
}

/// The key a project's split is kept under: its project file.
fn project_key(project: &Path) -> String {
    project.display().to_string()
}

/// The folder a project's documents are kept relative to.
fn project_folder(project: &Path) -> &Path {
    project.parent().unwrap_or(Path::new(""))
}
