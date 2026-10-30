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
use super::project_scan::{designator_scope_of, shown_path};
use crate::app::project_sheets;
use crate::app::sheet_hierarchy::build_sheet_hierarchy;
use crate::find_replace::{
    ChildSheetSignature, FindMatch, FindReplaceMsg, ReplaceJob, ReplacePrep,
};

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

/// One sheet a replace changed, before the whole run is committed.
pub(super) struct Applied {
    pub(super) path: PathBuf,
    /// The sheet had no engine and this run loaded one for it.
    pub(super) tabless: bool,
    pub(super) hits: usize,
    pub(super) invalidation: Option<crate::schematic_runtime::RenderInvalidation>,
}

impl Signex {
    /// Abandon the replace waiting on the disk (if any) and every scan in
    /// flight, saying why in the Messages panel.
    pub(crate) fn cancel_replace_job(&mut self, why: &str) {
        if self.ui_state.find_replace.supersede().is_some() {
            crate::diagnostics::log_info(format!("Replace cancelled: {why}."));
        }
    }

    /// The open sheets' child-sheet placements, read live (memory only).
    pub(crate) fn child_sheet_signature(&self) -> ChildSheetSignature {
        self.document_state
            .engines
            .iter()
            .map(|(path, engine)| {
                let placements = engine
                    .document()
                    .child_sheets
                    .iter()
                    .map(|cs| (cs.uuid, cs.name.clone(), cs.filename.clone()))
                    .collect();
                (path.clone(), placements)
            })
            .collect()
    }

    /// Start a project Replace of `matches`: read what the disk holds, off
    /// the UI thread. Applied by [`Self::apply_project_replace`].
    pub(super) fn start_project_replace(&mut self, matches: Vec<FindMatch>) -> Task<Message> {
        if matches.is_empty() {
            return Task::none();
        }
        if self.ui_state.find_replace.is_busy() {
            crate::diagnostics::log_info(
                "Replace: a search or replace is still running; wait for it to finish.",
            );
            return Task::none();
        }
        let query = self.ui_state.find_replace.query.trim().to_string();
        let Some(inputs) = self.capture_scan_inputs(&query) else {
            crate::diagnostics::log_warning("Replace: there is no project to replace in.");
            return Task::none();
        };
        let wanted = sheets_of(&matches);
        if self.refuse_replace_while_path_op(&inputs, &wanted) {
            return Task::none();
        }
        let epoch = self.document_state.documents_epoch;
        let child_sheets = self.child_sheet_signature();
        let find = &mut self.ui_state.find_replace;
        // Any later search change (a new generation) cancels this replace.
        find.scan_generation += 1;
        let generation = find.scan_generation;
        find.replace_job = Some(ReplaceJob {
            generation,
            epoch,
            child_sheets,
            query,
            replacement: find.replacement.clone(),
            matches: matches.clone(),
        });
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

    /// Refuse the Replace up front while a rename, delete or project-folder
    /// move holds a sheet it would read (`wanted`, the root or a declared
    /// page): the read bytes would be installed at a path that no longer
    /// exists (#859). Open sheets of other projects are not read, so they do
    /// not refuse it. True when refused.
    fn refuse_replace_while_path_op(
        &mut self,
        inputs: &super::project_scan::ScanInputs,
        wanted: &[PathBuf],
    ) -> bool {
        let sheets: Vec<&Path> = wanted
            .iter()
            .chain(&inputs.pages)
            .chain(std::iter::once(&inputs.root))
            .map(PathBuf::as_path)
            .collect();
        let refusal = match sheets.iter().find(|p| self.path_is_in_moving_project(p)) {
            Some(path) => format!(
                "{} is in a project whose folder is being renamed. Try again once it finishes.",
                path.display()
            ),
            None => match self.refuse_project_wide_while_path_op(sheets, "read") {
                Ok(()) => return false,
                Err(e) => e.to_string(),
            },
        };
        let detail = format!("Replace was not run: {refusal}");
        crate::diagnostics::log_warning(&detail);
        self.document_state.error_notice = Some(crate::app::state::ErrorNotice::replace(detail));
        true
    }

    /// Run the disk half on the calling thread - tests only. The job it
    /// leaves in flight is what a test then delivers (or not).
    #[cfg(test)]
    pub(crate) fn prepare_project_replace_blocking(
        &mut self,
        matches: Vec<FindMatch>,
    ) -> Option<(u64, ReplacePrep)> {
        let _ = self.start_project_replace(matches);
        let job = self.ui_state.find_replace.replace_job.clone()?;
        let inputs = self.capture_scan_inputs(&job.query)?;
        Some((job.generation, prepare(&inputs, &sheets_of(&job.matches))))
    }

    /// Run the disk half on the calling thread and apply - tests only.
    #[cfg(test)]
    pub(crate) fn run_project_replace_blocking(&mut self, matches: Vec<FindMatch>) {
        if let Some((generation, prep)) = self.prepare_project_replace_blocking(matches) {
            let _ = self.apply_project_replace(generation, &Ok(prep));
        }
    }

    /// The disk half arrived: apply every change, as one undo group.
    pub(super) fn apply_project_replace(
        &mut self,
        generation: u64,
        result: &Result<ReplacePrep, String>,
    ) -> Task<Message> {
        // Only the live job's own answer counts; a stale one must not take
        // the job a newer Replace is waiting on.
        let find = &mut self.ui_state.find_replace;
        let job = match find.replace_job.take() {
            Some(job) if job.generation == generation => job,
            other => {
                find.replace_job = other;
                return Task::none();
            }
        };
        if generation != find.scan_generation {
            crate::diagnostics::log_info("Replace cancelled: the search changed.");
            return Task::none();
        }
        if job.epoch != self.document_state.documents_epoch || self.replace_sheets_held(&job) {
            crate::diagnostics::log_warning(
                "Replace cancelled: a document changed while the sheets were being read. \
                 Nothing was changed; run Replace All again.",
            );
            return self.refresh_find_matches();
        }
        if job.child_sheets != self.child_sheet_signature() {
            crate::diagnostics::log_warning(
                "Replace cancelled: a sheet symbol was placed, removed or changed while the                  sheets were being read, so the designators' instances may differ.                  Nothing was changed; run Replace All again.",
            );
            return self.refresh_find_matches();
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

    /// A sheet with no engine that the replace would install one for is being
    /// written or moved by a job that is already running (Annotate's write):
    /// the bytes the replace read are no longer what the file holds.
    fn replace_sheets_held(&self, job: &ReplaceJob) -> bool {
        sheets_of(&job.matches).iter().any(|listed| {
            let path = self.resolve_held_spelling(listed.clone());
            !self.document_state.engines.contains_key(&path)
                && (self.document_state.document_saves.is_saving(&path)
                    || self.refuse_while_path_op(&path, "replaced").is_err())
        })
    }

    /// Apply `job` to every sheet it names, all or nothing: one `update`, one
    /// undo group, and a failure on any sheet puts every earlier one back.
    fn replace_across_sheets(&mut self, job: &ReplaceJob, prep: &ReplacePrep) {
        let group = self.mint_undo_group();
        let project_dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        let mut applied: Vec<Applied> = Vec::new();
        let mut left_alone = 0usize;

        for listed in sheets_of(&job.matches) {
            let path = self.resolve_held_spelling(listed.clone());
            if applied.iter().any(|a| a.path == path) {
                continue;
            }
            if let Some((_, why)) = prep.unreadable.iter().find(|(p, _)| *p == listed) {
                crate::diagnostics::log_warning(format!(
                    "Replace: skipped, {why}: {}",
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
                    "Replace: left unchanged, read-only (written by a newer Signex): {}",
                    path.display()
                ));
                self.drop_unchanged_tabless(&path, tabless);
                continue;
            }

            // A locked label or note is skipped and named, as the active
            // sheet's Replace does, instead of failing the whole run.
            let (commands, hits, dropped, skipped) =
                live_replacement(engine, job, prep, &path, &listed);
            left_alone += dropped;
            Self::report_locked_skips(&skipped);
            let outcome = engine.execute_batch_grouped(commands, Some(group));
            for warning in engine.take_warnings() {
                crate::diagnostics::log_warning(warning);
            }
            match outcome {
                Ok(result) if result.changed => applied.push(Applied {
                    path,
                    tabless,
                    hits,
                    invalidation: result
                        .patch_pair
                        .map(|pair| Self::render_invalidation_for_patch(pair.document)),
                }),
                Ok(_) => self.drop_unchanged_tabless(&path, tabless),
                Err(error) => {
                    crate::diagnostics::log_error(
                        &format!("Replace failed on '{}'", path.display()),
                        &anyhow::Error::new(error),
                    );
                    self.drop_unchanged_tabless(&path, tabless);
                    self.roll_back_replace(group, &applied);
                    crate::diagnostics::log_warning(format!(
                        "Replace stopped at {}; the {} sheet(s) already changed were put back. \
                         Nothing was replaced.",
                        shown_path(project_dir.as_deref(), &listed),
                        applied.len()
                    ));
                    return;
                }
            }
        }

        self.commit_replace(group, applied, project_dir.as_deref());
        if left_alone > 0 {
            crate::diagnostics::log_warning(format!(
                "Replace: {left_alone} result{} changed since the search and {} left alone.",
                if left_alone == 1 { "" } else { "s" },
                if left_alone == 1 { "was" } else { "were" },
            ));
        }
    }

    /// Put back every sheet in `applied`: pop the group's entry (it never
    /// reaches the redo stack) and drop the engines this run created. The
    /// replace's own batch already cleared each sheet's redo stack when it
    /// was recorded, so a rolled-back replace still costs the user the redo
    /// entries they had on those sheets.
    pub(super) fn roll_back_replace(
        &mut self,
        group: signex_engine::UndoGroupId,
        applied: &[Applied],
    ) {
        for done in applied {
            if let Some(engine) = self.document_state.engines.get_mut(&done.path)
                && let Err(error) = engine.discard_undo_group_top(group)
            {
                crate::diagnostics::log_error(
                    &format!("Replace: could not put back '{}'", done.path.display()),
                    &anyhow::Error::new(error),
                );
            }
            self.drop_unchanged_tabless(&done.path, done.tabless);
        }
    }

    /// Every sheet applied: mark them unsaved, register the group, refresh
    /// the canvas and the cached netlist, and say what happened.
    fn commit_replace(
        &mut self,
        group: signex_engine::UndoGroupId,
        applied: Vec<Applied>,
        project_dir: Option<&Path>,
    ) {
        let active = self.document_state.active_path.clone();
        let mut recorded = crate::app::UndoGroupSheets {
            kind: crate::app::UndoGroupKind::Replace,
            ..Default::default()
        };
        let mut active_invalidation = None;
        let mut replaced = 0usize;
        for done in &applied {
            replaced += done.hits;
            recorded.live.push(done.path.clone());
            if done.tabless {
                recorded.parked.push(done.path.clone());
            }
            if active.as_deref() == Some(done.path.as_path()) {
                active_invalidation = done.invalidation;
                self.ui_state.net_color_redo.remove(&done.path);
            } else {
                self.document_state.mark_document_dirty(&done.path);
            }
        }
        let names: Vec<String> = applied
            .iter()
            .map(|a| shown_path(project_dir, &a.path))
            .collect();
        self.set_undo_group_sheets(group, recorded);
        if let Some(invalidation) = active_invalidation {
            self.finish_schematic_mutation(invalidation, true, true);
        }
        self.refresh_panel_ctx();
        crate::diagnostics::log_info(if names.is_empty() {
            "Replaced 0 on 0 sheets".to_string()
        } else {
            format!(
                "Replaced {replaced} on {} sheet{}: {}",
                names.len(),
                if names.len() == 1 { "" } else { "s" },
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
                "Replace: skipped, could not be read: {}",
                path.display()
            ));
            return false;
        };
        match Engine::new_with_path(sheet.clone(), Some(path.to_path_buf())) {
            Ok(engine) => {
                self.document_state
                    .insert_engine(path.to_path_buf(), engine);
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
            self.document_state.remove_engine(path);
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
/// the sheet's live document, and how many hits they replace and how many shown hits
/// are no longer there (or no longer match) and were left alone.
pub(super) fn live_replacement(
    engine: &Engine,
    job: &ReplaceJob,
    prep: &ReplacePrep,
    path: &Path,
    listed: &Path,
) -> (Vec<Command>, usize, usize, Vec<String>) {
    let shown: HashSet<HitKey> = job
        .matches
        .iter()
        .filter(|m| m.sheet.as_deref() == Some(listed))
        .map(|m| hit_key(&m.hit))
        .collect();
    let scope = designator_scope_of(prep.hierarchy.as_ref(), true, path);
    let document = engine.document();
    let mut commands = Vec::new();
    let mut hits = 0;
    let mut locked = Vec::new();
    let mut still_there: HashSet<HitKey> = HashSet::new();
    for hit in signex_engine::scan_sheet(document, &job.query, &scope) {
        let key = hit_key(&hit);
        if !shown.contains(&key) {
            continue;
        }
        // A hit whose rewrite is a no-op (already the replacement's
        // spelling) is done, not stale.
        still_there.insert(key);
        let mine = signex_engine::replace_commands(document, &hit, &job.query, &job.replacement);
        // A hit on a locked label or note is skipped and named, and not
        // counted as replaced.
        let (mine, skipped) = Signex::without_locked_items(engine, mine);
        locked.extend(skipped);
        if !mine.is_empty() {
            hits += 1;
            commands.extend(mine);
        }
    }
    locked.sort();
    locked.dedup();
    (
        commands,
        hits,
        shown.len().saturating_sub(still_there.len()),
        locked,
    )
}
