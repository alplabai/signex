//! Navigator panel interaction: expand/collapse, highlight, open a sheet, and
//! ascend from the focused sheet to its parent.

use iced::Task;
use signex_widgets::tree_view::{TreeMsg, resolve_path, toggle};

use super::super::super::*;
use crate::app::handlers::canvas::{AfterOpen, SheetOpen};
use crate::app::sheet_hierarchy::SheetNodeKind;

impl Signex {
    /// `None` when `panel_msg` is not a Navigator message, so the dock
    /// dispatcher falls through to the next handler.
    pub(super) fn handle_dock_navigator_panel_message(
        &mut self,
        panel_msg: &crate::panels::PanelMsg,
    ) -> Option<Task<Message>> {
        let crate::panels::PanelMsg::Navigator(tree_msg) = panel_msg else {
            return None;
        };
        match tree_msg {
            TreeMsg::Toggle(key) => {
                let tree = &mut self.document_state.panel_ctx.navigator.tree;
                if let Some(path) = resolve_path(tree, key) {
                    toggle(tree, &path);
                }
            }
            TreeMsg::Select(key) => {
                let tree = &self.document_state.panel_ctx.navigator.tree;
                if let Some(path) = resolve_path(tree, key) {
                    return Some(self.navigator_row_clicked(&path));
                }
            }
            // No context menu is defined for sheet rows.
            TreeMsg::ContextMenu(_) | TreeMsg::BackgroundContextMenu => {}
        }
        Some(Task::none())
    }

    /// Single click highlights the row; a second click on it within 500 ms
    /// opens (or focuses) that sheet — the Projects tree's rule.
    fn navigator_row_clicked(&mut self, path: &[usize]) -> Task<Message> {
        // A row of the previous hierarchy would record a placement the project
        // may no longer have; the click is ignored until the rebuild lands.
        if !self.navigator_is_settled() {
            return Task::none();
        }
        let navigator = &mut self.document_state.panel_ctx.navigator;
        let Some(node) = navigator.node_at(path) else {
            return Task::none();
        };
        let placement = navigator
            .hierarchy
            .as_ref()
            .and_then(|hierarchy| hierarchy.placement_at(path));
        let (sheet_path, kind, label, detail, symbol) = (
            node.path.clone(),
            node.kind,
            node.name.clone(),
            node.detail.clone(),
            node.placed_as,
        );
        // The sheet that carries this row's symbol: one level up the tree.
        let parent_path = path
            .split_last()
            .filter(|(_, up)| !up.is_empty())
            .and_then(|(_, up)| navigator.node_at(up))
            .map(|parent| parent.path.clone());
        navigator.selected = Some(path.to_vec());
        if !navigator.register_click(path, std::time::Instant::now()) {
            return Task::none();
        }
        match kind {
            SheetNodeKind::Present => self.open_sheet_with_placement(sheet_path, placement),
            SheetNodeKind::Missing => {
                crate::diagnostics::log_warning(format!(
                    "Sheet '{label}' cannot be opened: {} is missing",
                    sheet_path.display()
                ));
                // Offer to create it, as the canvas does.
                let (Some(symbol), Some(parent)) = (symbol, parent_path) else {
                    return Task::none();
                };
                if self.active_tab_path().as_deref() == Some(parent.as_path()) {
                    self.open_create_child_sheet_dialog(symbol, true);
                    return Task::none();
                }
                self.open_sheet(
                    parent,
                    SheetOpen::plain().then(AfterOpen::OfferCreate { symbol }),
                )
            }
            SheetNodeKind::Unreadable => {
                crate::diagnostics::log_warning(format!(
                    "Sheet '{label}' cannot be opened: {} is unreadable ({})",
                    sheet_path.display(),
                    detail.as_deref().unwrap_or("no reason recorded")
                ));
                Task::none()
            }
            SheetNodeKind::Rejected => {
                crate::diagnostics::log_warning(format!(
                    "Sheet '{label}' cannot be opened: its reference '{}' points outside the sheet's folder",
                    sheet_path.display()
                ));
                Task::none()
            }
            SheetNodeKind::Cycle | SheetNodeKind::Truncated => {
                crate::diagnostics::log_warning(format!(
                    "Sheet '{label}' is not expanded here; open it from its first occurrence"
                ));
                Task::none()
            }
        }
    }

    /// Move from the focused sheet to the sheet that references it.
    ///
    /// Read from the cached Navigator hierarchy, never from disk. A file
    /// instantiated from several parents has several; the placement the tab
    /// was entered by names one, else the first by path is taken. The root,
    /// a sheet nothing references, and a project whose hierarchy is not
    /// cached are said in the Messages panel, not silently ignored. This only
    /// reads, so a hierarchy mid-rebuild is used as cached; a stale placement
    /// is rejected by `parent_through`.
    pub(crate) fn ascend_to_parent_sheet(&mut self) -> Task<Message> {
        let Some(active) = self.active_tab_path() else {
            crate::diagnostics::log_warning("Up Hierarchy: no sheet is focused");
            return Task::none();
        };
        if self.navigator_project_scope().is_none() {
            crate::diagnostics::log_warning(
                "Up Hierarchy: the focused sheet is not part of a loaded project",
            );
            return Task::none();
        }
        let step = {
            let navigator = &self.document_state.panel_ctx.navigator;
            let Some(hierarchy) = navigator.hierarchy.as_ref() else {
                let wording = if self.document_state.navigator_job.is_exhausted() {
                    "the project hierarchy could not be read"
                } else {
                    "the project hierarchy has not been read yet"
                };
                crate::diagnostics::log_warning(format!("Up Hierarchy: {wording}"));
                return Task::none();
            };
            // A recorded placement names the exact sheet symbol the user came
            // through, so it also names the parent, which then inherits the
            // placement one level up.
            let through = self
                .document_state
                .tab_placements
                .get(&active)
                .and_then(|placement| hierarchy.parent_through(placement, &active));
            match through {
                Some(step) => Some((SheetOpen::at(Some(step.placement.clone())), step)),
                None => hierarchy
                    .first_parent_of(&active)
                    .map(|step| (SheetOpen::plain(), step)),
            }
        };
        let Some((open, step)) = step else {
            let reason = if self.navigator_is_settled() {
                "it is the project root, no sheet references it, or it lies past the hierarchy's size limit"
            } else {
                "the project hierarchy is still being read"
            };
            crate::diagnostics::log_warning(format!(
                "Up Hierarchy: {} has no parent sheet: {reason}",
                active.display()
            ));
            return Task::none();
        };
        // Land on the sheet symbol that instantiates the sheet just left, once
        // the parent is showing.
        let then = AfterOpen::FocusSymbol {
            symbol: step.symbol,
        };
        self.open_sheet(step.parent, open.then(then))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use signex_types::format::SnxSchematic;
    use signex_widgets::tree_view::TreeMsg;

    use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
    use crate::app::*;
    use crate::panels::PanelMsg;

    /// Write `name` into `dir` as a real `.snxsch` whose sheet symbols
    /// reference `children` (as `(display name, file)`).
    fn write_sheet(dir: &Path, name: &str, children: &[(&str, &str)]) -> PathBuf {
        let files: Vec<&str> = children.iter().map(|(_, f)| *f).collect();
        let mut sheet = sheet_with_net("R1", "N", &files);
        for (cs, (display, _)) in sheet.child_sheets.iter_mut().zip(children) {
            cs.name = (*display).to_string();
        }
        let path = dir.join(name);
        let text = SnxSchematic::new(sheet).write_string().expect("encode");
        std::fs::write(&path, text).expect("write sheet");
        path
    }

    /// root -> Mid -> Leaf, all on disk; returns the app with the project
    /// loaded and `[root, mid, leaf]` paths.
    fn three_level_project(
        dir: &tempfile::TempDir,
        leaf_children: &[(&str, &str)],
    ) -> (Signex, [PathBuf; 3]) {
        let root = write_sheet(dir.path(), "root.snxsch", &[("Mid", "mid.snxsch")]);
        let mid = write_sheet(dir.path(), "mid.snxsch", &[("Leaf", "leaf.snxsch")]);
        let leaf = write_sheet(dir.path(), "leaf.snxsch", leaf_children);
        let app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
        (app, [root, mid, leaf])
    }

    fn active_tab(app: &Signex) -> Option<PathBuf> {
        app.document_state
            .tabs
            .get(app.document_state.active_tab)
            .map(|t| t.path.clone())
    }

    fn labels(app: &Signex, path: &[usize]) -> Vec<String> {
        let mut nodes = &app.document_state.panel_ctx.navigator.tree;
        let mut out = Vec::new();
        for &i in path {
            let node = &nodes[i];
            out.push(node.label.clone());
            nodes = &node.children;
        }
        out
    }

    #[test]
    fn navigator_shows_the_whole_hierarchy_even_with_a_leaf_focused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [_, _, leaf]) = three_level_project(&dir, &[]);

        app.open_or_focus_sheet_path(leaf.clone());
        app.refresh_panel_ctx();
        app.settle_navigator();

        assert_eq!(active_tab(&app), Some(leaf));
        assert_eq!(labels(&app, &[0, 0, 0]), ["root", "Mid", "Leaf"]);
        let tree = &app.document_state.panel_ctx.navigator.tree;
        assert!(tree[0].children[0].children[0].is_active, "leaf is marked");
        assert!(!tree[0].is_active);
    }

    #[test]
    fn a_reference_cycle_builds_a_finite_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, _) = three_level_project(&dir, &[("Back", "root.snxsch")]);

        app.refresh_panel_ctx();
        app.settle_navigator();

        assert_eq!(
            labels(&app, &[0, 0, 0, 0]),
            ["root", "Mid", "Leaf", "Back (cycle)"]
        );
    }

    #[test]
    fn double_click_opens_the_row_single_click_only_highlights() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [_, mid, _]) = three_level_project(&dir, &[]);
        app.refresh_panel_ctx();
        app.settle_navigator();
        let select = PanelMsg::Navigator(TreeMsg::Select(key_at(&app, &[0, 0])));

        click(&mut app, &select);
        assert!(
            app.document_state.tabs.is_empty(),
            "one click opens nothing"
        );
        assert_eq!(
            app.document_state.panel_ctx.navigator.selected,
            Some(vec![0, 0])
        );

        click(&mut app, &select);
        assert_eq!(active_tab(&app), Some(mid));
    }

    #[test]
    fn ascend_walks_from_the_leaf_up_to_the_root_and_stops_there() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [root, mid, leaf]) = three_level_project(&dir, &[]);
        app.open_or_focus_sheet_path(leaf);

        app.ascend_settled();
        assert_eq!(active_tab(&app), Some(mid));
        app.ascend_settled();
        assert_eq!(active_tab(&app), Some(root.clone()));
        app.ascend_settled();
        assert_eq!(active_tab(&app), Some(root), "the root has no parent");
    }

    fn key_at(app: &Signex, path: &[usize]) -> signex_widgets::tree_view::TreeKey {
        signex_widgets::tree_view::key_of(&app.document_state.panel_ctx.navigator.tree, path)
            .expect("row exists")
    }

    /// Send a Navigator message and finish the sheet read it starts.
    fn click(app: &mut Signex, message: &PanelMsg) {
        if let Some(task) = app.handle_dock_navigator_panel_message(message) {
            crate::app::handlers::canvas::sheet_open_tests::finish_read(app, task);
        }
    }

    fn double_click(app: &mut Signex, path: &[usize]) {
        let select = PanelMsg::Navigator(TreeMsg::Select(key_at(app, path)));
        click(app, &select);
        // A future memo instant saturates `duration_since` to zero, so the
        // second click is inside the window however loaded the machine is.
        app.document_state
            .panel_ctx
            .navigator
            .last_click
            .restamp(std::time::Instant::now() + std::time::Duration::from_secs(3600));
        click(app, &select);
    }

    #[test]
    fn a_double_click_while_the_hierarchy_rebuilds_opens_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [_, mid, _]) = three_level_project(&dir, &[]);
        app.refresh_panel_ctx();
        app.settle_navigator();

        // Opening a tab changes what the Navigator is built from.
        app.open_or_focus_sheet_path(dir.path().join("leaf.snxsch"));
        app.refresh_panel_ctx();
        assert!(!app.navigator_is_settled(), "precondition: rebuild pending");

        double_click(&mut app, &[0, 0]);

        assert!(
            !app.document_state.engines.contains_key(&mid),
            "the row of the previous hierarchy must not open a sheet"
        );
    }

    #[test]
    fn a_dot_dot_reference_opens_the_already_open_tab_not_a_second_engine() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = write_sheet(dir.path(), "root.snxsch", &[("Mid", "sub/../mid.snxsch")]);
        let mid = write_sheet(dir.path(), "mid.snxsch", &[]);
        let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
        app.open_or_focus_sheet_path(mid.clone());
        app.refresh_panel_ctx();
        app.settle_navigator();
        assert!(root.exists());

        double_click(&mut app, &[0, 0]);

        assert_eq!(app.document_state.tabs.len(), 1, "no second tab");
        assert_eq!(app.document_state.engines.len(), 1, "no second engine");
        assert_eq!(active_tab(&app), Some(mid));
    }

    #[test]
    fn escaping_and_absolute_references_cannot_be_opened() {
        let outer = tempfile::tempdir().expect("tempdir");
        let dir = outer.path().join("proj");
        std::fs::create_dir_all(&dir).expect("project dir");
        write_sheet(outer.path(), "x.snxsch", &[]);
        let abs = write_sheet(outer.path(), "abs.snxsch", &[]);
        let abs_ref = abs.to_string_lossy().into_owned();
        write_sheet(
            &dir,
            "root.snxsch",
            &[("Up", "../x.snxsch"), ("Abs", abs_ref.as_str())],
        );
        let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
        app.refresh_panel_ctx();
        app.settle_navigator();

        assert_eq!(labels(&app, &[0, 0]), ["root", "Up (rejected)"]);
        assert_eq!(labels(&app, &[0, 1]), ["root", "Abs (rejected)"]);
        double_click(&mut app, &[0, 0]);
        double_click(&mut app, &[0, 1]);

        assert!(app.document_state.tabs.is_empty(), "nothing was opened");
    }

    #[test]
    fn a_cycle_spelled_through_dot_dot_in_another_folder_is_marked_at_depth_two() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).expect("sub dir");
        write_sheet(dir.path(), "root.snxsch", &[("Mid", "sub/mid.snxsch")]);
        write_sheet(&sub, "mid.snxsch", &[("Again", "deep/../mid.snxsch")]);
        let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);

        app.refresh_panel_ctx();
        app.settle_navigator();

        assert_eq!(labels(&app, &[0, 0, 0]), ["root", "Mid", "Again (cycle)"]);
    }

    #[test]
    fn descending_through_the_navigator_records_the_clicked_placement() {
        use crate::app::handlers::menu::export::tests::current_placement::{
            PLACEMENT_B, current_page, placed_twice_on_disk, placement_text,
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, _top, child) = placed_twice_on_disk(&dir);
        app.refresh_panel_ctx();
        app.settle_navigator();

        double_click(&mut app, &[0, 1]);

        assert_eq!(
            current_page(&app, &child),
            ("R101".into(), placement_text(PLACEMENT_B))
        );
    }

    #[test]
    fn a_replaced_sheet_symbol_is_recorded_by_its_new_uuid() {
        use crate::app::handlers::menu::export::tests::current_placement::placed_twice_on_disk;
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, top, child) = placed_twice_on_disk(&dir);
        app.refresh_panel_ctx();
        app.settle_navigator();
        let engine = app
            .document_state
            .engines
            .get_mut(&top)
            .expect("top engine");
        let mut document = engine.document().clone();
        let replacement = uuid::Uuid::from_u128(77);
        document.child_sheets[1].uuid = replacement;
        *engine = signex_engine::Engine::new(document).expect("engine");
        app.refresh_panel_ctx();
        app.settle_navigator();

        double_click(&mut app, &[0, 1]);

        let recorded = app.document_state.tab_placements.get(&child);
        assert_eq!(
            recorded.map(ToString::to_string),
            Some(format!("/{replacement}"))
        );
    }

    #[test]
    fn ascend_resolves_the_parent_from_the_cached_hierarchy_not_the_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [_, mid, leaf]) = three_level_project(&dir, &[]);
        app.open_or_focus_sheet_path(leaf);
        app.settle_hierarchy();
        // The cache still places Leaf under Mid; the file no longer does, so
        // only a disk read of the closed sheets would lose the parent.
        write_sheet(dir.path(), "mid.snxsch", &[]);

        let task = app.ascend_to_parent_sheet();
        crate::app::handlers::canvas::sheet_open_tests::finish_read(&mut app, task);

        assert_eq!(active_tab(&app), Some(mid));
    }

    #[test]
    fn ascend_right_after_descending_does_not_wait_for_the_rebuild() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [root, mid, _]) = three_level_project(&dir, &[]);
        app.open_or_focus_sheet_path(root.clone());
        app.refresh_panel_ctx();
        app.settle_navigator();

        double_click(&mut app, &[0, 0]);
        app.refresh_panel_ctx();
        assert_eq!(active_tab(&app), Some(mid), "descended into Mid");
        assert!(!app.navigator_is_settled(), "precondition: rebuild pending");

        let task = app.ascend_to_parent_sheet();
        crate::app::handlers::canvas::sheet_open_tests::finish_read(&mut app, task);

        assert_eq!(active_tab(&app), Some(root));
    }

    #[test]
    fn up_hierarchy_outside_a_project_says_so() {
        let mut app = app_workspace("/nowhere", &[]);
        app.ascend_settled();
        assert!(app.document_state.tabs.is_empty());
    }
}
