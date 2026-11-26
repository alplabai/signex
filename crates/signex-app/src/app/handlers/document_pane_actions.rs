//! The pane strips, the ⋯ pane menu and the splitters of the document area
//! (mockup row 207, #1307), and the pane commands: Tile Grid, Move Tab to
//! Other Pane, Close Pane and Equal Pane Sizes. Each command returns why it
//! was refused, so the menu row, the palette and the agent channel say the
//! same thing. The rules live in `crate::document_panes`; Sync Zoom and the
//! view each pane keeps are in `pane_views`. Design: docs/MENU_BAR.md,
//! "Panes".

use iced::Task;
use signex_i18n::{ids, t};

use super::document_panes::TiledPanes;
use crate::app::{Message, PaneMsg, Signex, Tile, TileLayout};
use crate::diagnostics::log_info;
use crate::document_panes::{
    PaneShares, PaneSplit, SplitRefusal, geometry, grid_groups, pane_tiles, splitters,
};
use crate::menu_bar::MenuMessage;
use crate::menu_bar::model::{TilePreview, TilePreviews};

/// A pane command: the `Task` it leaves, or why it was refused.
pub(crate) type PaneOutcome = Result<Task<Message>, SplitRefusal>;

impl Signex {
    pub(crate) fn dispatch_pane_message(&mut self, msg: PaneMsg) -> Task<Message> {
        match msg {
            PaneMsg::SplitRequested { edge, second_view } => {
                self.split_active_document_pane(edge, second_view)
            }
            PaneMsg::Kept(result) => self.document_panes_kept(result),
            PaneMsg::TabPressed { pane, tab } => self.press_pane_tab(pane, tab),
            PaneMsg::MenuToggled(pane) => {
                let menu = &mut self.ui_state.workspace_layouts.panes.menu;
                *menu = (*menu != Some(pane)).then_some(pane);
                Task::none()
            }
            PaneMsg::MenuDismissed => {
                self.ui_state.workspace_layouts.panes.menu = None;
                Task::none()
            }
            PaneMsg::MenuCommandPicked { pane, command } => {
                self.ui_state.workspace_layouts.panes.menu = None;
                let focused = self.focus_tile_pane(pane);
                let run = Task::done(Message::Menu(MenuMessage::Command(command)));
                Task::batch([focused, run])
            }
            PaneMsg::MoveTabRequested => self.run_pane_command(Self::move_tab_to_other_pane),
            PaneMsg::CloseRequested => self.run_pane_command(Self::close_focused_pane),
            PaneMsg::SyncZoomRequested => self.run_pane_command(Self::sync_pane_zoom),
            PaneMsg::EqualizeRequested => self.run_pane_command(Self::equalize_panes),
            PaneMsg::SplitterPressed(index) => {
                self.ui_state.workspace_layouts.panes.splitter_drag = Some(index);
                Task::none()
            }
            PaneMsg::TabReleased(window) => self.release_tab_over_document_area(window),
        }
    }

    /// A dragged tab was let go over a widget that took the release. Over
    /// the main window's document area the drag ends there, splitting on a
    /// pane edge; anywhere else (the tab strip reorders on its own release)
    /// nothing happens here.
    fn release_tab_over_document_area(&mut self, released_in: iced::window::Id) -> Task<Message> {
        if self.ui_state.tab_dragging.is_none() || self.ui_state.main_window_id != Some(released_in)
        {
            return Task::none();
        }
        let (x, y) = self.interaction_state.last_mouse_pos;
        if !self.document_area_in(None).contains(iced::Point::new(x, y)) {
            return Task::none();
        }
        let drop = self.pane_drop_on_release(released_in);
        self.handle_layout_drag_finished();
        self.interaction_state.pointer_window = None;
        drop.map_or_else(Task::none, |drop| self.drop_tab_on_pane_edge(drop))
    }

    /// Run pane command `run`; a refusal is said in Messages.
    fn run_pane_command(&mut self, run: fn(&mut Self) -> PaneOutcome) -> Task<Message> {
        run(self).unwrap_or_else(|why| self.say_split_refused(why))
    }

    /// The pane sizes of `view`: those of the last splitter drag when they
    /// were made for these panes, else empty (equal).
    pub(crate) fn pane_shares_for(&self, view: &TiledPanes) -> &[f32] {
        self.ui_state
            .workspace_layouts
            .panes
            .shares
            .as_ref()
            .map_or(&[][..], |shares| {
                shares.for_view(view.layout, view.tabs.len())
            })
    }

    /// Where each pane of `view` sits in a document area of `size`,
    /// relative to its top-left; its tab strip is the top of the tile.
    pub(crate) fn pane_cells(&self, view: &TiledPanes, size: (f32, f32)) -> Vec<Tile> {
        pane_tiles(
            view.tabs.len(),
            size,
            view.layout,
            self.pane_shares_for(view),
        )
    }

    /// View ▸ Window ▸ Tile Grid in the main window: up to four panes, one
    /// document kind each where it can (`grid_groups`), kept for the
    /// project. Whether panes were made, and the write of the kept split.
    pub(crate) fn tile_documents_grid(&mut self) -> (bool, Task<Message>) {
        let (_, keys) = self.docked_pane_tabs();
        let Some(active) = self.active_pane_tab().filter(|_| keys.len() >= 2) else {
            return (false, Task::none());
        };
        let kinds: Vec<&str> = keys.iter().map(|key| key.page.as_str()).collect();
        let groups = grid_groups(&kinds);
        let Some(split) = PaneSplit::from_groups(&keys, &groups, TileLayout::Grid, &active) else {
            return (false, Task::none());
        };
        let before = self.canvas_bounds();
        let panes = &mut self.ui_state.workspace_layouts.panes;
        panes.split = Some(split);
        panes.shares = None;
        self.ui_state.document_tiling = Some(TileLayout::Grid);
        self.settle_document_tiling();
        self.follow_main_canvas_resize(before);
        let shown = self.tiled_pane_view().map_or(0, |view| view.tabs.len());
        let line = t!(
            self.ui_state.i18n,
            ids::WORKSPACE_LAYOUTS_PANES_GRID_DONE,
            "count" => keys.len(),
            "panes" => shown
        );
        log_info(line.into_string());
        (true, self.keep_document_panes())
    }

    /// What the tile rows of View ▸ Window preview while hovered (mockup row
    /// 207): the documents each pane would receive. None with fewer than
    /// two documents in the main window.
    pub(crate) fn tile_previews(&self) -> TilePreviews {
        let (docked, keys) = self.docked_pane_tabs();
        if docked.len() < 2 {
            return TilePreviews::default();
        }
        let names: Vec<String> = docked
            .iter()
            .filter_map(|&index| self.document_state.tabs.get(index))
            .map(|tab| tab.title.clone())
            .collect();
        let i18n = &self.ui_state.i18n;
        let one_each: Vec<Vec<String>> = names.iter().map(|name| vec![name.clone()]).collect();
        let kinds: Vec<&str> = keys.iter().map(|key| key.page.as_str()).collect();
        let grid: Vec<Vec<String>> = grid_groups(&kinds)
            .iter()
            .map(|group| {
                group
                    .iter()
                    .filter_map(|&at| names.get(at).cloned())
                    .collect()
            })
            .collect();
        let preview = |id: &str, layout: TileLayout, panes: Vec<Vec<String>>, note| TilePreview {
            layout,
            title: t!(
                i18n,
                ids::WORKSPACE_LAYOUTS_PANES_PREVIEW_TITLE,
                "layout" => crate::menu_bar::cmd_label(i18n, id, id),
                "count" => names.len()
            )
            .into_string(),
            panes,
            note: t!(i18n, note).into_string(),
        };
        let each = ids::WORKSPACE_LAYOUTS_PANES_PREVIEW_ONE_EACH;
        TilePreviews {
            horizontal: Some(preview(
                "tile_windows_horizontally",
                TileLayout::Stacked,
                one_each.clone(),
                each,
            )),
            vertical: Some(preview(
                "tile_windows_vertically",
                TileLayout::SideBySide,
                one_each,
                each,
            )),
            grid: Some(preview(
                "tile_windows_grid",
                TileLayout::Grid,
                grid,
                ids::WORKSPACE_LAYOUTS_PANES_PREVIEW_GRID,
            )),
        }
    }

    /// Tile Vertically or Horizontally's one pane per document as a split
    /// with the same panes, so a pane command can move tabs between them.
    fn materialize_pane_split(&mut self) -> Result<(), SplitRefusal> {
        if self.ui_state.workspace_layouts.panes.split.is_some() {
            return Ok(());
        }
        let layout = self
            .ui_state
            .document_tiling
            .ok_or(SplitRefusal::NotTiled)?;
        let (_, keys) = self.docked_pane_tabs();
        let active = self.active_pane_tab().ok_or(SplitRefusal::NotDocked)?;
        let groups: Vec<Vec<usize>> = (0..keys.len()).map(|at| vec![at]).collect();
        let split = PaneSplit::from_groups(&keys, &groups, layout, &active)
            .ok_or(SplitRefusal::NotTiled)?;
        self.ui_state.workspace_layouts.panes.split = Some(split);
        Ok(())
    }

    /// A tab on the strip of shown pane `pane` was pressed: the pane takes
    /// the focus and shows tab `tab`, which becomes the active one.
    fn press_pane_tab(&mut self, pane: usize, tab: usize) -> Task<Message> {
        if self.ui_state.workspace_layouts.panes.split.is_none() {
            // One pane per document: the strip's tab is the pane's.
            return self.focus_tile_pane(pane);
        }
        let (docked, keys) = self.docked_pane_tabs();
        let Some(at) = docked.iter().position(|&index| index == tab) else {
            return Task::none();
        };
        let before = self.canvas_bounds();
        if let Some(split) = self.ui_state.workspace_layouts.panes.split.as_mut() {
            let shown = split.resolve(&keys);
            if let Some(index) = shown
                .get(pane)
                .filter(|shown| shown.tabs.contains(&at))
                .map(|shown| shown.pane)
            {
                split.focused = index;
                split.panes[index].shown = Some(keys[at].clone());
            }
        }
        if tab == self.document_state.active_tab {
            self.settle_document_tiling();
        } else {
            self.park_active_schematic_session();
            self.document_state.active_tab = tab;
            self.sync_active_tab();
        }
        self.follow_main_canvas_resize(before);
        Task::none()
    }

    /// After the panes changed: they settle, and the main canvases follow
    /// the focused pane's new size.
    fn settle_after_pane_change(&mut self, before: iced::Rectangle) {
        self.settle_document_tiling();
        self.follow_main_canvas_resize(before);
    }

    /// Move Tab to Other Pane: the active tab moves to the next pane, which
    /// takes the focus; kept for the project.
    pub(crate) fn move_tab_to_other_pane(&mut self) -> PaneOutcome {
        if self.tiled_pane_view().is_none() {
            return Err(SplitRefusal::NotTiled);
        }
        self.materialize_pane_split()?;
        let (_, keys) = self.docked_pane_tabs();
        let tab = self.active_pane_tab().ok_or(SplitRefusal::NotDocked)?;
        let before = self.canvas_bounds();
        let split = self
            .ui_state
            .workspace_layouts
            .panes
            .split
            .as_mut()
            .ok_or(SplitRefusal::NotTiled)?;
        split.move_tab(&keys, &tab)?;
        self.settle_after_pane_change(before);
        Ok(self.keep_document_panes())
    }

    /// Close Pane: the focused pane closes and its tabs join the next one;
    /// no document closes. Down to one pane the split ends and the project
    /// forgets it, as Untile does.
    pub(crate) fn close_focused_pane(&mut self) -> PaneOutcome {
        let view = self.tiled_pane_view().ok_or(SplitRefusal::NotTiled)?;
        self.materialize_pane_split()?;
        let (_, keys) = self.docked_pane_tabs();
        let before = self.canvas_bounds();
        let split = self
            .ui_state
            .workspace_layouts
            .panes
            .split
            .as_mut()
            .ok_or(SplitRefusal::NotTiled)?;
        if split.close_pane(&keys, view.focused)? {
            self.settle_after_pane_change(before);
            return Ok(self.keep_document_panes());
        }
        let panes = &mut self.ui_state.workspace_layouts.panes;
        panes.split = None;
        panes.shares = None;
        self.ui_state.document_tiling = None;
        self.settle_after_pane_change(before);
        Ok(self.keep_document_panes())
    }

    /// Equal Pane Sizes, and a double click on a splitter: the panes share
    /// the document area equally again.
    pub(crate) fn equalize_panes(&mut self) -> PaneOutcome {
        if self.tiled_pane_view().is_none() {
            return Err(SplitRefusal::NotTiled);
        }
        let before = self.canvas_bounds();
        self.ui_state.workspace_layouts.panes.shares = None;
        self.follow_main_canvas_resize(before);
        Ok(self.keep_document_panes())
    }

    /// The pointer moved to (`x`, `y`) while a splitter is dragged: the
    /// panes on either side of it take the new sizes.
    pub(crate) fn drag_pane_splitter(&mut self, x: f32, y: f32) {
        let Some(index) = self.ui_state.workspace_layouts.panes.splitter_drag else {
            return;
        };
        let Some(view) = self.tiled_pane_view() else {
            return;
        };
        let area = self.document_area_in(None);
        let size = (area.width, area.height);
        let count = view.tabs.len();
        let current = self.pane_shares_for(&view).to_vec();
        let Some(splitter) = splitters(count, size, view.layout, &current)
            .into_iter()
            .find(|splitter| splitter.index == index)
        else {
            return;
        };
        let at = if splitter.vertical {
            x - area.x
        } else {
            y - area.y
        };
        let Some(values) = geometry::dragged(count, size, view.layout, &current, index, at) else {
            return;
        };
        let before = self.canvas_bounds();
        self.ui_state.workspace_layouts.panes.shares = Some(PaneShares {
            layout: view.layout,
            count,
            values,
        });
        self.follow_main_canvas_resize(before);
    }

    /// A splitter drag ended: the sizes are kept with the project's panes.
    pub(crate) fn end_pane_splitter_drag(&mut self) -> Task<Message> {
        if self
            .ui_state
            .workspace_layouts
            .panes
            .splitter_drag
            .take()
            .is_none()
        {
            return Task::none();
        }
        self.keep_document_panes()
    }
}
