//! The compiled design of each project (#1308): what `compile_project` left
//! in memory, and the compile in flight. Never written to disk. The panel
//! and channel-tab state that goes with it is UI state
//! (`app::compiled_design::CompiledDesignUi`).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use signex_net::{CompiledDesign, SheetKey, SheetMiniature};

use super::project_id::ProjectId;

/// One project's last compile.
#[derive(Debug, Clone)]
pub struct CompiledResult {
    pub design: Arc<CompiledDesign>,
    /// The file each sheet key was read from.
    pub paths: HashMap<SheetKey, PathBuf>,
    /// When the compile finished; the view model words it in the user's
    /// locale (#2733).
    pub compiled_at: DateTime<Utc>,
    /// The card miniature of each channel row, by row index of `design`
    /// (#2658).
    pub miniatures: HashMap<usize, Arc<SheetMiniature>>,
    /// Sheets edited since the compile.
    pub stale: BTreeSet<PathBuf>,
    /// A sheet could not be read, or a sheet symbol places a missing file, so
    /// a page missing from `design` is not known to be removed (#2742).
    pub has_unread_sheets: bool,
    /// Those sheets, as the compile would have read them.
    pub unread_paths: Vec<PathBuf>,
    /// Bare contents links the compile found no file for beside their table.
    pub bare_links_missing: Vec<PathBuf>,
}

impl CompiledResult {
    /// Whether the compile read the sheet at `path`.
    pub fn covers(&self, path: &Path) -> bool {
        self.paths.values().any(|p| p == path)
    }

    /// The file a compiled row was read from.
    pub fn path_of(&self, key: &SheetKey) -> Option<&PathBuf> {
        self.paths.get(key)
    }
}

/// Every project's last compile and the compile in flight.
#[derive(Debug, Default)]
pub struct CompiledDesigns {
    pub results: HashMap<ProjectId, CompiledResult>,
    /// The compile in flight: its project and generation.
    pub pending: Option<(ProjectId, u64)>,
    /// Bumped by every compile request; an answer for an older one is
    /// dropped.
    pub generation: u64,
}
