//! Open-tree-document + tree-path resolution for the project-navigation dock.
//!
//! Extracted verbatim from the project-navigation dock handlers
//! (`handlers/dock/project_navigation`); pure code motion, zero
//! behaviour change.

use super::*;
use anyhow::{Context, Result};

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
            let entry = project.data.libraries.iter().find(|e| {
                e.path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|n| n == raw_label)
                    .unwrap_or(false)
            })?;
            return Some(project.data.resolve_library_path(entry));
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
    ) -> Result<()> {
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
            let entry = loaded
                .data
                .libraries
                .iter()
                .find(|entry| {
                    entry
                        .path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|n| n == filename)
                        .unwrap_or(false)
                })
                .with_context(|| {
                    format!(
                        "library {} not registered on project {}",
                        filename,
                        loaded.path.display()
                    )
                })?;
            loaded.data.resolve_library_path(entry)
        } else {
            project_dir.join(&filename)
        };

        // F23 — orphan `.snxlib` entries (registered on the project but
        // missing on disk) still flow through to the Library Browser
        // tab so the user sees a "Library not mounted" message and
        // can pick Remove from Project to clean up. Other extensions
        // still error early because they have no recovery surface.
        if !file_path.exists() {
            if filename.ends_with(".snxlib") {
                let _ = self.handle_open_library_browser(file_path);
                return Ok(());
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
            return Ok(());
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
                return Ok(());
            }
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read schematic {}", file_path.display()))?;
            let schematic = signex_types::format::SnxSchematic::parse(&text)
                .with_context(|| format!("parse schematic {}", file_path.display()))?
                .sheet;
            self.open_schematic_tab(file_path, title, schematic);
            return Ok(());
        }

        if filename.ends_with(".snxpcb") {
            let text = std::fs::read_to_string(&file_path)
                .with_context(|| format!("read pcb {}", file_path.display()))?;
            let board = signex_types::format::SnxPcb::parse(&text)
                .with_context(|| format!("parse pcb {}", file_path.display()))?
                .board;
            let title = filename.trim_end_matches(".snxpcb").to_string();
            self.open_pcb_tab(file_path, title, board);
            return Ok(());
        }

        // Standalone primitive editor tabs — `.snxsym` / `.snxfpt`
        // route through the library subsystem so the same
        // `OpenPrimitiveEditor` path used by the Library panel
        // right-click handles project-tree double-clicks too.
        if filename.ends_with(".snxsym") || filename.ends_with(".snxfpt") {
            let _ = self.handle_open_primitive(file_path);
            return Ok(());
        }

        // `.snxlib/` is a directory package, not a document. Open it
        // as a Library Browser tab in the main canvas area — the
        // browser is the primary surface for working with library rows
        // (table grid + symbol/footprint preview).
        if filename.ends_with(".snxlib") {
            // The browser handler returns a Task; in this synchronous
            // path we can drop it because mount + open are all
            // immediate-side-effecting (no async file dialogs etc.).
            let _ = self.handle_open_library_browser(file_path);
            return Ok(());
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
