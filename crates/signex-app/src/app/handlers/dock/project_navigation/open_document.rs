//! Open-tree-document + tree-path resolution for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;
use anyhow::{Context, Result};
use iced::Task;

/// The registered library whose resolved `.snxlib` path is `key` (a library
/// row's tree key). Never falls back to the file name: two libraries may
/// share one (#742).
fn library_path_for_key(
    data: &signex_types::project::ProjectData,
    key: &str,
) -> Option<std::path::PathBuf> {
    data.libraries
        .iter()
        .map(|entry| data.resolve_library_path(entry))
        .find(|path| path.to_string_lossy() == key)
}

impl Signex {
    /// Resolve a project-tree path (indices) to the file path on disk
    /// for the leaf node at that position. Multi-root aware: the first
    /// index picks which project's directory to resolve against, so a
    /// leaf under project B isn't accidentally resolved against project
    /// A's parent directory.
    ///
    /// F22 follow-up: `.snxlib` library leaves can live outside the
    /// project directory (`LibraryEntryKind::Shared`). Resolve those
    /// through `ProjectData::resolve_library_path` instead of joining
    /// the leaf label against the project dir; otherwise the assembled
    /// path doesn't exist and downstream remove / open paths bail
    /// silently.
    pub(super) fn tree_path_to_file_path(&self, tree_path: &[usize]) -> Option<std::path::PathBuf> {
        let node = signex_widgets::tree_view::get_node(
            self.document_state.panel_ctx.project_tree.as_slice(),
            tree_path,
        )?;
        let project_idx = *tree_path.first()?;
        let project = self.document_state.projects.get(project_idx)?;
        // F24 — `build_project_tree` appends "  (missing)" to leaf
        // labels when the backing file is absent from disk. Strip
        // that suffix here so filename-matching against
        // `entry.path.file_name()` still works on orphan rows.
        let raw_label = canonical_tree_label(&node.label);
        if raw_label.ends_with(".snxlib") {
            return library_path_for_key(&project.data, &node.key);
        }
        let dir = project.path.parent()?;
        // HI-5: refuse to feed `dir.join(raw_label)` if `raw_label` is
        // a path that escapes the project root. The label is a UI
        // string (canonical_tree_label) but a user could craft an
        // entry via "Add Existing" with `..` or a path separator;
        // this then flows into `reveal_in_file_manager` which dispatches
        // `xdg-open` on Linux (MIME-type-driven) and `explorer /select,`
        // on Windows. Reject anything that isn't a single path
        // component free of `..`.
        let raw_path = std::path::Path::new(raw_label);
        if raw_path.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return None;
        }
        Some(dir.join(raw_label))
    }

    pub(super) fn open_project_tree_document(
        &mut self,
        tree_path: &[usize],
        filename: String,
    ) -> Result<Task<Message>> {
        // F24 — strip the "  (missing)" suffix `build_project_tree`
        // appends to orphan rows so filename-matching downstream
        // still resolves the correct entry.
        let filename = canonical_tree_label(&filename).to_string();
        // Multi-project: walk to the owning project via tree_path[0]
        // instead of the active project, so clicking a leaf inside
        // project B opens B's file even when A is the active project.
        // (#54)
        let project_idx = *tree_path
            .first()
            .with_context(|| format!("project tree path was empty for {}", filename))?;
        let loaded = self
            .document_state
            .projects
            .get(project_idx)
            .with_context(|| format!("resolve project for {}", filename))?;
        let project_dir = loaded
            .path
            .parent()
            .with_context(|| format!("resolve project directory for {}", filename))?
            .to_path_buf();

        // F21 follow-up — `.snxlib` library entries can live outside
        // the project directory (`LibraryEntryKind::Shared` when the
        // user picked a destination outside `project_dir` in the
        // New Library save-as dialog). The legacy `project_dir.join
        // (filename)` reconstruction silently broke for those because
        // the assembled path didn't exist on disk → the bail at the
        // bottom fired and the double-click looked dead.
        //
        // For `.snxlib` we resolve through `project.data.libraries`
        // by matching the entry's filename, then ask
        // `ProjectData::resolve_library_path` which returns the
        // canonical absolute path (project-local entries are joined
        // against `project.dir`; shared / global entries are passed
        // through). Other extensions still use the filename-relative
        // path because schematics / pcbs / primitives have always
        // lived inside the project directory.
        let file_path: std::path::PathBuf = if filename.ends_with(".snxlib") {
            // By the row's key (the library's resolved path), so two
            // libraries sharing a file name stay distinct (#742).
            let key = signex_widgets::tree_view::get_node(
                self.document_state.panel_ctx.project_tree.as_slice(),
                tree_path,
            )
            .map(|node| node.key.as_str());
            key.and_then(|key| library_path_for_key(&loaded.data, key))
                .with_context(|| {
                    format!(
                        "library {} not registered on project {}",
                        filename,
                        loaded.path.display()
                    )
                })?
        } else {
            project_dir.join(&filename)
        };
        // One document per file: a spelling of the path a tab already holds
        // reuses that tab (#1149).
        let file_path = self.resolve_held_spelling(file_path);

        // F23 — orphan `.snxlib` entries (registered on the project but
        // missing on disk) still flow through to the Library Browser
        // tab so the user sees a "Library not mounted" message and
        // can pick Remove from Project to clean up. Other extensions
        // still error early because they have no recovery surface.
        if !file_path.exists() {
            if filename.ends_with(".snxlib") {
                return Ok(self.handle_open_library_browser(file_path));
            }
            anyhow::bail!("project tree file does not exist: {}", file_path.display());
        }

        if let Some(index) = self
            .document_state
            .tabs
            .iter()
            .position(|tab| tab.path == file_path)
        {
            if index != self.document_state.active_tab {
                self.park_active_schematic_session();
                self.document_state.active_tab = index;
                self.sync_active_tab();
            }
            return Ok(Task::none());
        }

        if filename.ends_with(".snxsch") {
            let title = filename.trim_end_matches(".snxsch").to_string();
            // If we parked an engine for this file (closed while dirty),
            // restore it instead of reparsing — Altium parity. Re-parsing
            // would silently discard the user's in-memory edits, which is
            // exactly what `dirty_paths` was built to prevent.
            if self.document_state.engines.contains_key(&file_path)
                && self.document_state.dirty_paths.contains(&file_path)
            {
                self.attach_parked_schematic_tab(file_path, title);
                return Ok(Task::none());
            }
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read schematic {}", file_path.display()))?;
            let schematic = signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", file_path.display()))?
                .sheet;
            self.open_schematic_tab(file_path, title, schematic);
            return Ok(Task::none());
        }

        if filename.ends_with(".snxpcb") {
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read pcb {}", file_path.display()))?;
            let board = signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", file_path.display()))?
                .board;
            let title = filename.trim_end_matches(".snxpcb").to_string();
            self.open_pcb_tab(file_path, title, board);
            return Ok(Task::none());
        }

        // Standalone primitive editor tabs — `.snxsym` / `.snxfpt`
        // route through the library subsystem so the same
        // `OpenPrimitiveEditor` path used by the Library panel
        // right-click handles project-tree double-clicks too.
        if filename.ends_with(".snxsym") || filename.ends_with(".snxfpt") {
            return Ok(self.handle_open_primitive(file_path));
        }

        // `.snxlib/` is a directory package, not a document. Open it
        // as a Library Browser tab in the main canvas area — the
        // browser is the primary surface for working with library rows
        // (table grid + symbol/footprint preview).
        if filename.ends_with(".snxlib") {
            return Ok(self.handle_open_library_browser(file_path));
        }

        anyhow::bail!("unsupported project tree document: {filename}")
    }
}

/// F24 — strip the "  (missing)" suffix `build_project_tree` appends
/// to leaves whose backing file is absent from disk, so downstream
/// filename matching against `entry.path.file_name()` still works.
/// Returns the original `&str` when no suffix is present (zero-copy).
fn canonical_tree_label(label: &str) -> &str {
    label.trim_end_matches("  (missing)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Signex;

    /// Regression (#99 part 1): `open_project_tree_document` used to
    /// return `Result<()>` and the `.snxsym` / `.snxlib` branches
    /// discarded the `Task` from `handle_open_primitive` /
    /// `handle_open_library_browser` via `let _ = ...`. The signature
    /// now carries the `Task` out — the explicit `Task<Message>`
    /// annotation below is a compile-time tripwire: this test stops
    /// compiling (rather than merely failing) if that return type is
    /// ever swallowed back down to `Result<()>`.
    ///
    /// A Task-*value* assertion isn't meaningful here: every callee on
    /// this path returns `Task::none()` unconditionally today (async
    /// `.snxlib` mounting lands in part 2 of #99), so the propagated
    /// and dropped cases are runtime-indistinguishable until then.
    #[test]
    fn open_project_tree_document_snxsym_carries_the_task_out() {
        let (mut app, _bootstrap_task) = Signex::new();

        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("proj.snxprj");
        std::fs::write(&project_path, b"").unwrap();
        // Content doesn't need to parse — `handle_open_primitive`
        // returns `Task::none()` on a parse failure too, and this test
        // only needs the file to exist so the tree-document lookup
        // doesn't bail with "does not exist".
        std::fs::write(dir.path().join("Comp.snxsym"), b"").unwrap();

        // Named binding, not `drop(..)` (walks past the #577 / #627 gate) and
        // not an assertion: what the project-open Task carries is not what
        // this test is about.
        let _opened_task: iced::Task<Message> = app.handle_document_file_opened(Some(project_path));
        assert_eq!(
            app.document_state.projects.len(),
            1,
            "project must have loaded before resolving a tree document against it"
        );

        let result: anyhow::Result<iced::Task<Message>> =
            app.open_project_tree_document(&[0], "Comp.snxsym".to_string());
        assert!(result.is_ok(), "expected Ok(Task), got {result:?}");
    }

    /// #1149 — the project tree resolves `dir/Top.snxsch`; a tab that holds
    /// the same file as `dir/sub/../Top.snxsch` is activated, not duplicated.
    #[test]
    fn project_tree_open_under_another_spelling_reuses_the_tab() {
        let (mut app, _bootstrap_task) = Signex::new();
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("proj.snxprj");
        std::fs::write(&project_path, b"").unwrap();
        let sheet_file = dir.path().join("Top.snxsch");
        std::fs::write(&sheet_file, b"").unwrap();
        let _opened_task: iced::Task<Message> = app.handle_document_file_opened(Some(project_path));

        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let held = dir.path().join("sub").join("..").join("Top.snxsch");
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "Top".to_string(),
            path: held.clone(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "Other".to_string(),
            path: dir.path().join("Other.snxsch"),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 1;

        let result = app.open_project_tree_document(&[0], "Top.snxsch".to_string());

        assert!(result.is_ok(), "expected Ok(Task), got {result:?}");
        assert_eq!(app.document_state.tabs.len(), 2);
        assert_eq!(app.document_state.active_tab, 0, "the held tab is focused");
        assert_eq!(app.document_state.tabs[0].path, held);
    }

    /// #1208 — a sheet opened from the project tree must be checked for an
    /// earlier run's recovery copy like File > Open: until the check
    /// resolves, autosave may not write over that copy.
    #[test]
    fn a_sheet_opened_from_the_project_tree_holds_autosave_back_until_its_recovery_check() {
        let (mut app, _bootstrap_task) = Signex::new();
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path().join("proj.snxprj");
        std::fs::write(&project_path, b"").unwrap();
        let sheet = signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::new_v4(),
            paper_size: "A4".to_string(),
            ..Default::default()
        };
        let text = signex_engine::Engine::new(sheet)
            .unwrap()
            .serialize_with_net_colors(&app.ui_state.wire_color_overrides)
            .unwrap();
        std::fs::write(dir.path().join("Top.snxsch"), text).unwrap();
        app.document_state
            .autosave
            .set_dir(Some(dir.path().join("recovery")));
        let _opened_task: iced::Task<Message> = app.handle_document_file_opened(Some(project_path));

        let result = app.open_project_tree_document(&[0], "Top.snxsch".to_string());

        assert!(result.is_ok(), "expected Ok(Task), got {result:?}");
        let opened = app
            .document_state
            .tabs
            .last()
            .expect("a tab opened")
            .path
            .clone();
        let stamp = crate::autosave::DocStamp {
            revision: 1,
            net_color_revision: 0,
        };
        assert!(
            !app.document_state.autosave.needs_write(&opened, stamp),
            "autosave must wait for the recovery check of a freshly opened sheet"
        );
    }
}
