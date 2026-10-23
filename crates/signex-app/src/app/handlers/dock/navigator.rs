//! Navigator panel interaction: expand/collapse, highlight, open a sheet, and
//! ascend from the focused sheet to its parent.

use iced::Task;
use signex_widgets::tree_view::{TreeMsg, resolve_path, toggle};

use super::super::super::*;
use crate::app::project_sheets::assemble_project_sheets;
use crate::app::sheet_hierarchy::{SheetNodeKind, parent_sheet};

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
                    self.navigator_row_clicked(&path);
                }
            }
            // No context menu is defined for sheet rows.
            TreeMsg::ContextMenu(_) | TreeMsg::BackgroundContextMenu => {}
        }
        Some(Task::none())
    }

    /// Single click highlights the row; a second click on it within 500 ms
    /// opens (or focuses) that sheet — the Projects tree's rule.
    fn navigator_row_clicked(&mut self, path: &[usize]) {
        let navigator = &mut self.document_state.panel_ctx.navigator;
        let Some(node) = navigator.node_at(path) else {
            return;
        };
        let (sheet_path, kind, label, detail) = (
            node.path.clone(),
            node.kind,
            node.name.clone(),
            node.detail.clone(),
        );
        navigator.selected = Some(path.to_vec());
        if !navigator.register_click(path, std::time::Instant::now()) {
            return;
        }
        match kind {
            SheetNodeKind::Present => self.open_or_focus_sheet_path(sheet_path),
            SheetNodeKind::Missing => crate::diagnostics::log_warning(format!(
                "Sheet '{label}' cannot be opened: {} is missing",
                sheet_path.display()
            )),
            SheetNodeKind::Unreadable => crate::diagnostics::log_warning(format!(
                "Sheet '{label}' cannot be opened: {} is unreadable ({})",
                sheet_path.display(),
                detail.as_deref().unwrap_or("no reason recorded")
            )),
            SheetNodeKind::Rejected => crate::diagnostics::log_warning(format!(
                "Sheet '{label}' cannot be opened: its reference '{}' points outside the sheet's folder",
                sheet_path.display()
            )),
            SheetNodeKind::Cycle | SheetNodeKind::Truncated => {
                crate::diagnostics::log_warning(format!(
                    "Sheet '{label}' is not expanded here; open it from its first occurrence"
                ));
            }
        }
    }

    /// Move from the focused sheet to the sheet that references it.
    ///
    /// A file instantiated from several parents has several; the first by
    /// path is taken (see [`parent_sheet`]). The root, and a sheet nothing
    /// references, have no parent — said in the Messages panel, not silently
    /// ignored.
    pub(crate) fn ascend_to_parent_sheet(&mut self) {
        let Some(active) = self.active_tab_path() else {
            crate::diagnostics::log_warning("Up Hierarchy: no sheet is focused");
            return;
        };
        let Some((root, pages)) = self.navigator_project_scope() else {
            crate::diagnostics::log_warning(
                "Up Hierarchy: the focused sheet is not part of a loaded project",
            );
            return;
        };
        let set = assemble_project_sheets(&self.document_state, &pages, &root);
        match parent_sheet(&set.sheets, &active) {
            Some(parent) => self.open_or_focus_sheet_path(parent),
            None => crate::diagnostics::log_warning(format!(
                "{} has no parent sheet: it is the project root or no sheet references it",
                active.display()
            )),
        }
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
        let select = PanelMsg::Navigator(TreeMsg::Select(key_at(&app, &[0, 0])));

        let _ = app.handle_dock_navigator_panel_message(&select);
        assert!(
            app.document_state.tabs.is_empty(),
            "one click opens nothing"
        );
        assert_eq!(
            app.document_state.panel_ctx.navigator.selected,
            Some(vec![0, 0])
        );

        let _ = app.handle_dock_navigator_panel_message(&select);
        assert_eq!(active_tab(&app), Some(mid));
    }

    #[test]
    fn ascend_walks_from_the_leaf_up_to_the_root_and_stops_there() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, [root, mid, leaf]) = three_level_project(&dir, &[]);
        app.open_or_focus_sheet_path(leaf);

        app.ascend_to_parent_sheet();
        assert_eq!(active_tab(&app), Some(mid));
        app.ascend_to_parent_sheet();
        assert_eq!(active_tab(&app), Some(root.clone()));
        app.ascend_to_parent_sheet();
        assert_eq!(active_tab(&app), Some(root), "the root has no parent");
    }

    fn key_at(app: &Signex, path: &[usize]) -> signex_widgets::tree_view::TreeKey {
        signex_widgets::tree_view::key_of(&app.document_state.panel_ctx.navigator.tree, path)
            .expect("row exists")
    }

    fn double_click(app: &mut Signex, path: &[usize]) {
        let select = PanelMsg::Navigator(TreeMsg::Select(key_at(app, path)));
        let _ = app.handle_dock_navigator_panel_message(&select);
        let _ = app.handle_dock_navigator_panel_message(&select);
    }

    #[test]
    fn a_dot_dot_reference_opens_the_already_open_tab_not_a_second_engine() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = write_sheet(dir.path(), "root.snxsch", &[("Mid", "sub/../mid.snxsch")]);
        let mid = write_sheet(dir.path(), "mid.snxsch", &[]);
        let mut app = app_workspace(&dir.path().to_string_lossy(), &["root.snxsch"]);
        app.open_or_focus_sheet_path(mid.clone());
        app.refresh_panel_ctx();
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

        assert_eq!(labels(&app, &[0, 0, 0]), ["root", "Mid", "Again (cycle)"]);
    }

    #[test]
    fn up_hierarchy_outside_a_project_says_so() {
        let mut app = app_workspace("/nowhere", &[]);
        app.ascend_to_parent_sheet();
        assert!(app.document_state.tabs.is_empty());
    }
}
