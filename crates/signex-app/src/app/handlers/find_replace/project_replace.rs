//! Project-scope Replace: every change on every sheet is one undo group.
//!
//! The disk is read in a `Task` (the sheets no engine holds, and a fresh
//! hierarchy); every edit is then applied in ONE `update`, so no other
//! message can land between the first sheet and the last.
//!
//! Data-loss rules:
//! - a closed sheet is loaded into a tabless engine and left dirty in memory,
//!   exactly as a parked dirty sheet is; nothing is written to disk;
//! - the text to replace is re-read from each sheet's live document, and only
//!   hits the user was shown are replaced;
//! - a sheet is looked up through `resolve_held_spelling`, so one file never
//!   ends up with two engines;
//! - a read-only or unreadable sheet is skipped and named in the Messages
//!   panel.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use signex_engine::{Command, Engine};

use super::super::super::*;
use super::project_scan::designator_scope_of;
use crate::app::project_sheets;
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::find_replace::{FindMatch, FindReplaceMsg, ReplaceJob, ReplacePrep};

/// Identity of a hit across a rescan: the item and the placement it was read at.
type HitKey = (signex_types::schematic::SelectedItem, Option<String>);

fn hit_key(hit: &signex_engine::FindHit) -> HitKey {
    (hit.item, hit.placement.as_ref().map(ToString::to_string))
}

/// The distinct sheets of `matches`, in first-seen order.
fn sheets_of(matches: &[FindMatch]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    matches
        .iter()
        .filter_map(|m| m.sheet.as_deref())
        .filter(|p| seen.insert(p.to_path_buf()))
        .map(Path::to_path_buf)
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Signex {
    /// Start a project Replace of `matches`: read what the disk holds, off
    /// the UI thread. Applied by [`Self::apply_project_replace`].
    pub(super) fn start_project_replace(&mut self, matches: Vec<FindMatch>) -> Task<Message> {
        if matches.is_empty() {
            return Task::none();
        }
        let query = self.ui_state.find_replace.query.trim().to_string();
        let Some(inputs) = self.capture_scan_inputs(&query) else {
            crate::diagnostics::log_warning("Replace: there is no project to replace in.");
            return Task::none();
        };
        let find = &mut self.ui_state.find_replace;
        // Any later search change (a new generation) cancels this replace.
        find.scan_generation += 1;
        let generation = find.scan_generation;
        find.replace_job = Some(ReplaceJob {
            generation,
            query,
            replacement: find.replacement.clone(),
            matches: matches.clone(),
        });
        let wanted = sheets_of(&matches);
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || prepare(&inputs, &wanted))
                    .await
                    .map_err(|e| format!("Replace worker failed: {e}"))
            },
            move |result| {
                Message::FindReplaceMsg(FindReplaceMsg::ProjectReplacePrepared {
                    generation,
                    result: Arc::new(result),
                })
            },
        )
    }

    /// Run the disk half on the calling thread and apply - tests only.
    #[cfg(test)]
    pub(crate) fn run_project_replace_blocking(&mut self, matches: Vec<FindMatch>) {
        let _ = self.start_project_replace(matches);
        let Some(job) = self.ui_state.find_replace.replace_job.clone() else {
            return;
        };
        let wanted = sheets_of(&job.matches);
        let inputs = self
            .capture_scan_inputs(&job.query)
            .expect("a project to replace in");
        let prep = prepare(&inputs, &wanted);
        let _ = self.apply_project_replace(job.generation, &Ok(prep));
    }

    /// The disk half arrived: apply every change, as one undo group.
    pub(super) fn apply_project_replace(
        &mut self,
        generation: u64,
        result: &Result<ReplacePrep, String>,
    ) -> Task<Message> {
        let Some(job) = self.ui_state.find_replace.replace_job.take() else {
            return Task::none();
        };
        if job.generation != generation || generation != self.ui_state.find_replace.scan_generation
        {
            crate::diagnostics::log_info(
                "Replace cancelled: the search changed while the sheets were being read.",
            );
            return Task::none();
        }
        let prep = match result {
            Ok(prep) => prep,
            Err(why) => {
                crate::diagnostics::log_warning(why);
                return Task::none();
            }
        };
        self.replace_across_sheets(&job, prep);
        self.refresh_find_matches()
    }

    /// Apply `job` to every sheet it names. One `update`, one undo group.
    fn replace_across_sheets(&mut self, job: &ReplaceJob, prep: &ReplacePrep) {
        let group = self.mint_undo_group();
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind: crate::app::UndoGroupKind::Replace,
            ..Default::default()
        };
        let mut replaced = 0usize;
        let mut background_changed = false;
        let mut active_invalidation = None;

        for listed in sheets_of(&job.matches) {
            let path = self.resolve_held_spelling(listed.clone());
            if recorded.live.contains(&path) {
                continue;
            }
            if let Some((_, why)) = prep.unreadable.iter().find(|(p, _)| *p == listed) {
                crate::diagnostics::log_warning(format!(
                    "Replace: sheet {} was skipped: {why}",
                    listed.display()
                ));
                continue;
            }
            let tabless = !self.document_state.engines.contains_key(&path);
            if tabless && !self.install_tabless_engine(&path, prep.loaded.get(&listed)) {
                continue;
            }
            let Some(engine) = self.document_state.engines.get_mut(&path) else {
                continue;
            };
            if engine.document().unknown.newer_minor_source.is_some() {
                crate::diagnostics::log_warning(format!(
                    "Replace: sheet {} was written by a newer Signex, is read-only in this \
                     build, and was left unchanged.",
                    path.display()
                ));
                self.drop_unchanged_tabless(&path, tabless);
                continue;
            }

            let (commands, hits) = live_replacement(engine, job, prep, &path, &listed);
            let outcome = engine.execute_batch_grouped(commands, Some(group));
            for warning in engine.take_warnings() {
                crate::diagnostics::log_warning(warning);
            }
            match outcome {
                Ok(result) if result.changed => {
                    replaced += hits;
                    recorded.live.push(path.clone());
                    if active.as_deref() == Some(path.as_path()) {
                        active_invalidation = result
                            .patch_pair
                            .map(|pair| Self::render_invalidation_for_patch(pair.document));
                        self.ui_state.net_color_redo.remove(&path);
                    } else {
                        self.mark_cached_sheet_dirty(&path);
                        background_changed = true;
                    }
                }
                Ok(_) => self.drop_unchanged_tabless(&path, tabless),
                Err(error) => {
                    crate::diagnostics::log_error(
                        &format!("Replace failed on '{}'", path.display()),
                        &anyhow::Error::new(error),
                    );
                    self.drop_unchanged_tabless(&path, tabless);
                }
            }
        }

        let names: Vec<String> = recorded.live.iter().map(|p| file_name(p)).collect();
        self.set_undo_group_sheets(group, recorded);
        if let Some(invalidation) = active_invalidation {
            self.finish_schematic_mutation(invalidation, true, true);
        }
        // A label changed on a sheet that is not the active one, and only the
        // active sheet's own edit refreshes the cached project netlist.
        if background_changed {
            self.ui_state.project_netlist = None;
            self.refresh_project_netlist();
        }
        self.refresh_panel_ctx();
        crate::diagnostics::log_info(if names.is_empty() {
            "Replaced 0 on 0 sheets".to_string()
        } else {
            format!(
                "Replaced {replaced} on {} sheets: {}",
                names.len(),
                names.join(", ")
            )
        });
    }

    /// A closed sheet's engine: tabless, adopting its net colours, held until
    /// the user saves or discards. `false` (and a Messages line) when the sheet
    /// is not usable.
    fn install_tabless_engine(
        &mut self,
        path: &Path,
        loaded: Option<&signex_types::schematic::SchematicSheet>,
    ) -> bool {
        let Some(sheet) = loaded else {
            crate::diagnostics::log_warning(format!(
                "Replace: sheet {} could not be read and was skipped.",
                path.display()
            ));
            return false;
        };
        match Engine::new_with_path(sheet.clone(), Some(path.to_path_buf())) {
            Ok(engine) => {
                self.document_state
                    .engines
                    .insert(path.to_path_buf(), engine);
                self.adopt_net_colors_of(path);
                self.clear_net_color_steps(path);
                true
            }
            Err(error) => {
                crate::diagnostics::log_error(
                    &format!("Replace: sheet {} could not be loaded", path.display()),
                    &anyhow::Error::new(error),
                );
                false
            }
        }
    }

    /// An engine loaded only to be edited, left clean and tabless, would own
    /// the path for the rest of the session and shadow a later open.
    fn drop_unchanged_tabless(&mut self, path: &Path, tabless: bool) {
        if tabless {
            self.forget_net_colors_of(path);
            self.document_state.engines.remove(path);
        }
    }
}

/// The disk half: sheets no engine holds, plus a hierarchy built now.
fn prepare(inputs: &super::project_scan::ScanInputs, wanted: &[PathBuf]) -> ReplacePrep {
    let set = project_sheets::assemble_project_sheets_from(
        &inputs.open,
        &inputs.pages,
        &inputs.root,
        inputs.fold,
    );
    let hierarchy = build_sheet_hierarchy(
        &set.sheets,
        &set.unreadable,
        &inputs.root,
        &set.pages_outside_the_hierarchy,
        inputs.fold,
    );
    let loaded = wanted
        .iter()
        .filter(|p| !inputs.open.contains_key(*p))
        .filter_map(|p| set.sheets.get(p).map(|s| (p.clone(), s.clone())))
        .collect();
    ReplacePrep {
        hierarchy: Some(hierarchy),
        loaded,
        unreadable: set.unreadable,
    }
}

/// The commands for the hits the user was shown on `listed`, found again in
/// the sheet's live document, and how many hits they replace.
fn live_replacement(
    engine: &Engine,
    job: &ReplaceJob,
    prep: &ReplacePrep,
    path: &Path,
    listed: &Path,
) -> (Vec<Command>, usize) {
    let shown: HashSet<HitKey> = job
        .matches
        .iter()
        .filter(|m| m.sheet.as_deref() == Some(listed))
        .map(|m| hit_key(&m.hit))
        .collect();
    let scope = designator_scope_of(prep.hierarchy.as_ref(), path);
    let document = engine.document();
    let mut commands = Vec::new();
    let mut hits = 0;
    for hit in signex_engine::scan_sheet(document, &job.query, &scope) {
        if !shown.contains(&hit_key(&hit)) {
            continue;
        }
        let mine = signex_engine::replace_commands(document, &hit, &job.query, &job.replacement);
        if !mine.is_empty() {
            hits += 1;
            commands.extend(mine);
        }
    }
    (commands, hits)
}
