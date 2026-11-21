//! A schematic tab opened, reattached, shown or left.

use std::path::{Path, PathBuf};

use crate::app::state::DocumentState;
use crate::app::{TabInfo, TabKind};

impl DocumentState {
    /// Push a schematic tab for `path` and make it the active one. Its
    /// engine is not touched: the caller installs or points at one next.
    pub(crate) fn register_schematic_tab(&mut self, path: PathBuf, title: String) {
        let project_id = self.project_for_path(&path).map(|p| p.id);
        self.tabs.push(TabInfo {
            title,
            path,
            cached_document: None,
            project_id,
            kind: TabKind::Schematic,
        });
        self.active_tab = self.tabs.len() - 1;
    }

    /// A sheet read from disk was installed (`installed`) at `path`: its
    /// history position is the clean point, unless a repair (`repaired`)
    /// left it differing from its file (#1830). Autosave is held for it and
    /// its recovery check queued (#1208); the per-placement designator note
    /// is due.
    pub(crate) fn settle_opened_sheet(&mut self, path: &Path, installed: bool, repaired: bool) {
        // A failed install leaves the older engine at `path`, which is not
        // known to equal the file.
        if installed && let Some(engine) = self.engines.get_mut(path) {
            engine.mark_saved();
        }
        if repaired && installed {
            self.mark_document_dirty(path);
        }
        self.instance_note_due = true;
        self.autosave.begin_check(path);
        self.recovery_check_due.push(path.to_path_buf());
    }

    /// Reopen the parked engine at `path` (closed while dirty, #1135) in a
    /// new active tab: the engine and its edits stay as they are.
    pub(crate) fn reattach_parked_sheet(&mut self, path: PathBuf, title: String) {
        self.register_schematic_tab(path.clone(), title);
        self.active_path = Some(path);
    }

    /// Point `active_path` at the active tab's engine when one exists;
    /// whether it now does.
    pub(crate) fn activate_tab_engine(&mut self) -> bool {
        let path = self.active_tab_path();
        if let Some(p) = path.as_ref()
            && self.engines.contains_key(p)
        {
            self.active_path = path;
            true
        } else {
            self.active_path = None;
            false
        }
    }

    /// Detach from the visible schematic engine and leave it in `engines`:
    /// switching back to a parked sheet (Schematic → PCB → Schematic) must
    /// find it resident. Only closing a tab removes an engine (#51).
    pub(crate) fn detach_active_schematic(&mut self) {
        self.active_path = None;
    }
}
