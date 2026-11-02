//! Questions the stock-copy flow asks of the open libraries (#813).

use std::path::{Path, PathBuf};

use super::LibraryState;
use crate::library::stock_copy::StockPrimitiveKind;

/// The pinned `library_id` of the bundled stock library (#323).
pub const STOCK_LIBRARY_ID: uuid::Uuid = uuid::uuid!("019df45d-af5e-7a00-8c3b-dd3bf89b40ae");

impl LibraryState {
    /// Refuse a library at `root` that is a copy of the bundled stock
    /// library (it carries the stock id but is not the stock file), so the
    /// bundled one always wins, whichever mount finishes first (#1894).
    pub(crate) fn refuse_stock_copy(
        &self,
        root: &Path,
        library_id: uuid::Uuid,
    ) -> Result<(), signex_library::LibraryError> {
        // Only while the bundled stock is mounted or still mounting: when it
        // failed to open (#814), a copy is all that keeps stock references in
        // designs resolving, so it is let through.
        let stock_live = self.stock_library.is_some()
            || self.pending_mounts.keys().any(|p| self.is_stock_root(p));
        if library_id != STOCK_LIBRARY_ID || !stock_live || self.is_stock_root(root) {
            return Ok(());
        }
        Err(signex_library::LibraryError::Conflict(format!(
            "{} is a copy of the bundled stock library (same library id), so it was not opened. \
             To change the stock library itself, run Signex from a source checkout with \
             SIGNEX_EDIT_STOCK=1 (see docs/STOCK_LIBRARY.md)",
            root.display()
        )))
    }

    /// The kind of `path` when it is a primitive file inside the locked
    /// stock library; `None` for any other path, and for every path while
    /// a maintainer edits the stock library itself (#1892).
    pub(crate) fn stock_primitive_kind(&self, path: &Path) -> Option<StockPrimitiveKind> {
        let kind = StockPrimitiveKind::of_path(path)?;
        self.containing_library(path)
            .is_some_and(|lib| self.is_locked_stock_root(&lib.root))
            .then_some(kind)
    }

    /// Drop what the Components and Library panels hold for the closed
    /// library at `root`: its selection, details, row menu, drag, hover and
    /// expand state. Without this a closed library's details stay on screen
    /// and its row menu can still try Place or Open.
    pub(crate) fn forget_panel_state_of(&mut self, root: &Path) {
        let of = |key: &crate::library::state::ComponentsPanelRowKey| key.library_path == root;
        let panel = &mut self.components_panel;
        if panel.selected.as_ref().is_some_and(of) {
            panel.selected = None;
        }
        if panel.details.key.as_ref().is_some_and(of) {
            panel.details = Default::default();
        }
        if panel.row_menu.as_ref().is_some_and(|m| of(&m.key)) {
            panel.row_menu = None;
        }
        if panel.drag.as_ref().is_some_and(|d| of(&d.key)) {
            panel.drag = None;
        }
        if panel.hovered.as_ref().is_some_and(of) {
            panel.hovered = None;
        }
        self.panel_expanded.remove(root);
        if self.panel_selected.as_ref().is_some_and(of) {
            self.panel_selected = None;
        }
        if self.panel_last_click.as_ref().is_some_and(|(k, _)| of(k)) {
            self.panel_last_click = None;
        }
    }

    /// Mounted libraries that accept writes, as `(.snxlib path, name)`.
    pub fn writable_libraries(&self) -> Vec<(&Path, &str)> {
        self.open_libraries
            .iter()
            .filter(|lib| self.can_receive_stock_copy(&lib.root))
            .map(|lib| (lib.root.as_path(), lib.display_name.as_str()))
            .collect()
    }

    /// The remembered copy target, if that library is still mounted and
    /// writable; `None` means the user has to be asked.
    pub fn usable_stock_copy_target(&self) -> Option<&Path> {
        self.stock_copy_target
            .as_deref()
            .filter(|root| self.can_receive_stock_copy(root))
    }

    /// Whether the library at `root` is mounted, writable and not the
    /// stock library itself.
    pub fn can_receive_stock_copy(&self, root: &Path) -> bool {
        self.library_at(root)
            .is_some_and(|lib| lib.root_dir().is_some())
            && !self.is_read_only(root)
            && !self.is_stock_root(root)
    }

    /// What to call the stock file at `path` in the dialog: the name the
    /// library lists for it when the file name is its uuid, else the file
    /// name without extension.
    pub fn stock_primitive_label(&self, path: &Path, kind: StockPrimitiveKind) -> String {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(lib) = self.containing_library(path) else {
            return stem;
        };
        let summaries = match kind {
            StockPrimitiveKind::Symbol => &lib.cached_symbols,
            StockPrimitiveKind::Footprint => &lib.cached_footprints,
        };
        uuid::Uuid::parse_str(&stem)
            .ok()
            .and_then(|id| summaries.iter().find(|s| s.uuid == id))
            .map_or(stem, |s| s.name.clone())
    }

    /// The directory of the library `path` lives in, for resolving the
    /// STEP models its footprints attach.
    pub(crate) fn library_dir_of(&self, path: &Path) -> Option<PathBuf> {
        self.containing_library(path)
            .and_then(|lib| lib.root_dir())
            .map(Path::to_path_buf)
    }
}
