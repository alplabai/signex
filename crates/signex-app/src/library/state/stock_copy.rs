//! Questions the stock-copy flow asks of the open libraries (#813).

use std::path::{Path, PathBuf};

use super::LibraryState;
use crate::library::stock_copy::StockPrimitiveKind;

impl LibraryState {
    /// The kind of `path` when it is a primitive file inside the stock
    /// library, which mounts read-only; `None` for any other path.
    pub(crate) fn stock_primitive_kind(&self, path: &Path) -> Option<StockPrimitiveKind> {
        let kind = StockPrimitiveKind::of_path(path)?;
        self.containing_library(path)
            .is_some_and(|lib| self.is_stock_root(&lib.root))
            .then_some(kind)
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
