//! Sheets an edit brings into the agent step that is running (#2501): an
//! `apply_edits` list or an agent's one-step run. `sync_sheet_entries
//! direction=to_sheet` writes the child sheet, which the call did not name;
//! the child joins the step with a transaction of its own and commits under
//! the step's undo group, so one undo reverts the parent and the child. See
//! `docs/AGENT_EDITS.md`, "Sheets an edit brings in".

use std::path::{Path, PathBuf};

use signex_types::schematic::SchematicSheet;

use crate::app::state::JoinedSheet;
use crate::app::{Signex, UndoGroupKind, UndoGroupSheets};

impl Signex {
    /// Bring `path` into the running agent step, with the checks a grouped
    /// write makes (`sheet_write.rs`): a sheet no engine holds must not be
    /// being saved or moved and is loaded tabless from `loaded`, the copy the
    /// edit read; a read-only sheet is refused. A sheet the step already
    /// holds is left as it is. Nothing changes on a refusal. Answers the path
    /// the sheet is held under (an open sheet's own spelling).
    pub(crate) fn join_agent_step(
        &mut self,
        label: &str,
        listed: &Path,
        loaded: Option<&SchematicSheet>,
    ) -> Result<PathBuf, String> {
        let resolved = self.resolve_held_spelling(listed.to_path_buf());
        let path = resolved.as_path();
        let edits = &self.document_state.agent_edits;
        let held = edits.target.as_deref() == Some(path)
            || edits.touched.iter().any(|sheet| sheet == path)
            || edits.joined.iter().any(|sheet| sheet.path == path);
        if held {
            return Ok(resolved);
        }
        let tabless = !self.document_state.engines.contains_key(path);
        if tabless
            && (self.document_state.document_saves.is_saving(path)
                || self.refuse_while_path_op(path, "synced").is_err())
        {
            return Err(format!("{} is being saved or moved", path.display()));
        }
        let read_only = match self.document_state.engines.get(path) {
            Some(engine) => engine.document().is_read_only(),
            None => loaded.is_some_and(SchematicSheet::is_read_only),
        };
        if read_only {
            return Err(format!("{} opens read-only", path.display()));
        }
        if tabless && !self.install_tabless_engine(label, path, loaded) {
            return Err(format!("{} could not be loaded", path.display()));
        }
        let clean_before = !self.document_state.is_document_dirty(path);
        let begun = match self.document_state.engines.get_mut(path) {
            Some(engine) => engine.begin_transaction().map_err(|e| e.to_string()),
            None => Err(format!("{} could not be loaded", path.display())),
        };
        if let Err(why) = begun {
            self.drop_unchanged_tabless(path, tabless);
            return Err(why);
        }
        self.document_state.agent_edits.joined.push(JoinedSheet {
            path: path.to_path_buf(),
            tabless,
            clean_before,
        });
        Ok(resolved)
    }
}

/// The sheets the running step's edits brought in, taken out of the state.
pub(super) fn take_joined(app: &mut Signex) -> Vec<JoinedSheet> {
    std::mem::take(&mut app.document_state.agent_edits.joined)
}

/// The paths of `joined`.
pub(super) fn joined_paths(joined: &[JoinedSheet]) -> Vec<PathBuf> {
    joined.iter().map(|sheet| sheet.path.clone()).collect()
}

/// The step is rolled back: so is every joined sheet, and one loaded only for
/// it is dropped again.
pub(super) fn roll_back_joined(app: &mut Signex, joined: &[JoinedSheet]) {
    for sheet in joined {
        if let Some(engine) = app.document_state.engines.get_mut(&sheet.path) {
            engine.rollback_transaction();
        }
        app.drop_unchanged_tabless(&sheet.path, sheet.tabless);
    }
}

/// The step landed: a joined sheet loaded only for it that did not change is
/// dropped; when the step changed more than one sheet, or a joined one, it is
/// recorded as one cross-sheet group, a loaded sheet parked (clean again
/// after an undo).
pub(super) fn settle_joined(
    app: &mut Signex,
    group: signex_engine::UndoGroupId,
    changed: &[PathBuf],
    joined: &[JoinedSheet],
) {
    for sheet in joined.iter().filter(|s| !changed.contains(&s.path)) {
        app.drop_unchanged_tabless(&sheet.path, sheet.tabless);
    }
    let joined_changed = joined.iter().any(|s| changed.contains(&s.path));
    if changed.len() < 2 && !joined_changed {
        return;
    }
    let landed = |keep: fn(&JoinedSheet) -> bool| -> Vec<PathBuf> {
        (joined.iter())
            .filter(|s| keep(s) && changed.contains(&s.path))
            .map(|s| s.path.clone())
            .collect()
    };
    app.set_undo_group_sheets(
        group,
        UndoGroupSheets {
            live: changed.to_vec(),
            kind: UndoGroupKind::AgentEdit,
            parked: landed(|s| s.tabless),
            clean_before: landed(|s| s.clean_before),
            ..UndoGroupSheets::default()
        },
    );
}
