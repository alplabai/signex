//! The edits of one `apply_edits` call, run inside one engine transaction on
//! each sheet they touch and committed under one undo group (#2178). All or
//! nothing: a refusal rolls every sheet back, and a commit that fails after
//! another sheet's landed takes that one back too. See `docs/AGENT_EDITS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use iced::Task;
use serde_json::{Value, json};
use signex_engine::{CommandResult, Engine, EngineError, UndoGroupId};

use super::apply::{Prepared, Ran, file_name, refused};
use crate::app::{Message, Signex, UndoGroupKind, UndoGroupSheets};
use crate::schematic_runtime::RenderInvalidation;

use super::super::CommandOutcome;

/// Run `edits` on `sheets` (the call's own sheet is `call_path`). What their
/// handlers say in Messages is true only if the list lands, so those Info
/// lines are held and shown after the commit, or dropped with the rollback.
pub(super) fn run_group(
    app: &mut Signex,
    call_path: &Path,
    sheets: &[PathBuf],
    edits: &[Prepared],
    label: String,
) -> Ran {
    crate::diagnostics::hold_info();
    let ran = run_listed(app, call_path, sheets, edits, label);
    crate::diagnostics::release_held_info(matches!(ran.0, CommandOutcome::Answered(_)));
    ran
}

/// A transaction on every sheet, or none: a sheet that cannot begin (read-only,
/// busy) takes the ones begun before it back.
fn begin_all(app: &mut Signex, sheets: &[PathBuf]) -> Result<(), String> {
    for (index, path) in sheets.iter().enumerate() {
        let begun = match app.document_state.engines.get_mut(path) {
            Some(engine) => engine.begin_transaction().map_err(|e| e.to_string()),
            None => Err(format!("{} is not loaded", path.display())),
        };
        if let Err(why) = begun {
            rollback_all(app, &sheets[..index]);
            return Err(if sheets.len() > 1 {
                format!("{}: {why}", file_name(path))
            } else {
                why
            });
        }
    }
    Ok(())
}

fn rollback_all(app: &mut Signex, sheets: &[PathBuf]) {
    for path in sheets {
        if let Some(engine) = app.document_state.engines.get_mut(path) {
            engine.rollback_transaction();
        }
    }
}

/// What the edits answered and what they owe the screen.
struct Executed {
    tasks: Vec<Task<Message>>,
    /// What each edit answered, by index; `null` for one that only ran.
    results: Vec<Value>,
    owed: HashMap<PathBuf, RenderInvalidation>,
    failure: Option<String>,
}

/// Run every edit with the target set to the sheet it acts on. The refresh a
/// handler would do is owed per sheet until the group lands.
fn run_edits(app: &mut Signex, sheets: &[PathBuf], edits: &[Prepared]) -> Executed {
    app.document_state.agent_edits.touched = sheets.to_vec();
    app.document_state.agent_edits.listing = true;
    let mut ran = Executed {
        tasks: Vec::new(),
        results: Vec::new(),
        owed: HashMap::new(),
        failure: None,
    };
    for (index, edit) in edits.iter().enumerate() {
        let state = &mut app.document_state.agent_edits;
        state.target = Some(edit.path.clone());
        state.owed = RenderInvalidation::NONE;
        let done = (edit.handler)(app, &edit.args);
        let taken = std::mem::take(&mut app.document_state.agent_edits.owed);
        *ran.owed.entry(edit.path.clone()).or_default() |= taken;
        match done {
            (CommandOutcome::Ran, task) => {
                ran.tasks.push(task);
                ran.results.push(Value::Null);
            }
            (CommandOutcome::Answered(answer), task) => {
                ran.tasks.push(task);
                ran.results.push(answer);
            }
            (CommandOutcome::Disabled { reason }, _) => {
                ran.failure = Some(format!(
                    "edit {} (`{}`): {reason}",
                    index + 1,
                    edit.id.as_str()
                ));
                break;
            }
            (other, _) => {
                ran.failure = Some(format!(
                    "edit {} (`{}`) did not run: {other:?}",
                    index + 1,
                    edit.id.as_str()
                ));
                break;
            }
        }
    }
    let state = &mut app.document_state.agent_edits;
    state.target = None;
    state.listing = false;
    state.touched.clear();
    ran
}

/// Commit every sheet under `group` with `commit`, which returns whether the
/// sheet changed. When one fails after others landed, those are taken back
/// (`discard_undo_group_top`) and the rest rolled back. The take-back is not an
/// exact restore: the sheet's redo branch and any history the cap trimmed are
/// gone and its revision moved. `run_listed` checks every engine first, so a
/// multi-sheet call does not reach it.
pub(super) fn commit_all(
    app: &mut Signex,
    sheets: &[PathBuf],
    group: UndoGroupId,
    mut commit: impl FnMut(&mut Engine, usize, UndoGroupId) -> Result<CommandResult, EngineError>,
) -> Result<Vec<(PathBuf, bool)>, String> {
    let mut landed: Vec<(PathBuf, bool)> = Vec::new();
    for (index, path) in sheets.iter().enumerate() {
        let result = match app.document_state.engines.get_mut(path) {
            Some(engine) => commit(engine, index, group).map_err(|e| e.to_string()),
            None => Err(format!("{} is not loaded", path.display())),
        };
        match result {
            Ok(done) => landed.push((path.clone(), done.changed)),
            Err(why) => {
                undo_landed(app, &landed, group);
                rollback_all(app, &sheets[index..]);
                return Err(why);
            }
        }
    }
    Ok(landed)
}

/// Take back the entries of `group` that already landed on `landed`'s sheets.
fn undo_landed(app: &mut Signex, landed: &[(PathBuf, bool)], group: UndoGroupId) {
    for (path, _) in landed.iter().filter(|(_, changed)| *changed) {
        let Some(engine) = app.document_state.engines.get_mut(path) else {
            continue;
        };
        if let Err(error) = engine.discard_undo_group_top(group) {
            let error = anyhow::Error::new(error);
            crate::diagnostics::log_error(
                &format!(
                    "Agent edit: could not take back {} after a failed commit",
                    file_name(path)
                ),
                &error,
            );
        }
        // The revision moved with the take-back: dirtiness and the screen follow.
        app.after_sheet_change(path, RenderInvalidation::FULL);
    }
}

fn run_listed(
    app: &mut Signex,
    call_path: &Path,
    sheets: &[PathBuf],
    edits: &[Prepared],
    label: String,
) -> Ran {
    if let Err(reason) = begin_all(app, sheets) {
        return refused(reason);
    }
    let ran = run_edits(app, sheets, edits);
    if let Some(reason) = ran.failure {
        rollback_all(app, sheets);
        return refused(format!("{reason}. Nothing was applied"));
    }
    // With one sheet the commit itself refuses, and nothing has landed to take
    // back; with several, no engine may commit until every one can.
    let ready = sheets.len() < 2
        || sheets.iter().all(|path| {
            app.document_state
                .engines
                .get(path)
                .is_some_and(Engine::transaction_ready)
        });
    if !ready {
        rollback_all(app, sheets);
        return refused("a sheet's transaction cannot commit. Nothing was applied");
    }
    let group = app.mint_undo_group();
    let landed = match commit_all(app, sheets, group, |engine, _, group| {
        engine.commit_transaction(Some(group))
    }) {
        Ok(landed) => landed,
        Err(why) => {
            if sheets.len() > 1 {
                // Past the readiness check this does not happen.
                crate::diagnostics::log_error(
                    "Agent edit: a commit failed after every sheet was ready; sheets already committed were taken back and may have lost their redo history",
                    &anyhow::anyhow!(why.clone()),
                );
                debug_assert!(false, "a commit failed after the readiness check: {why}");
            }
            return refused(format!("{why}. Nothing was applied"));
        }
    };
    finish(app, call_path, group, &landed, ran, edits.len(), label)
}

/// The group landed: record the step, refresh what changed, answer.
fn finish(
    app: &mut Signex,
    call_path: &Path,
    group: UndoGroupId,
    landed: &[(PathBuf, bool)],
    ran: Executed,
    count: usize,
    label: String,
) -> Ran {
    // The notices the edits raised were held until now: they are real.
    for (path, _) in landed {
        if let Some(engine) = app.document_state.engines.get_mut(path) {
            Signex::report_engine_warnings(engine);
        }
    }
    let changed: Vec<PathBuf> = landed
        .iter()
        .filter(|(_, changed)| *changed)
        .map(|(path, _)| path.clone())
        .collect();
    let revision_of = |app: &Signex, path: &Path| {
        app.document_state
            .engines
            .get(path)
            .map_or(0, Engine::revision)
    };
    let revision = revision_of(app, call_path);
    let mut answer = if changed.is_empty() {
        json!({"changed": false, "revision": revision, "group": null, "edits": count, "results": ran.results})
    } else {
        app.document_state
            .agent_edits
            .record_step(group, label.clone(), &app.agent.origin);
        if changed.len() > 1 {
            app.set_undo_group_sheets(
                group,
                UndoGroupSheets {
                    live: changed.clone(),
                    kind: UndoGroupKind::AgentEdit,
                    ..UndoGroupSheets::default()
                },
            );
        }
        for path in &changed {
            let owed = ran.owed.get(path).copied().unwrap_or_default();
            let invalidation = if owed == RenderInvalidation::NONE {
                RenderInvalidation::FULL
            } else {
                owed
            };
            app.after_sheet_change(path, invalidation);
        }
        let names: Vec<String> = changed.iter().map(|path| file_name(path)).collect();
        crate::diagnostics::log_info(format!(
            "{label}: {count} edit(s) applied to {} (undo group {})",
            names.join(", "),
            group.get()
        ));
        json!({"changed": true, "revision": revision, "group": group.get(), "label": label, "edits": count, "results": ran.results})
    };
    // A call that touched anything but its own sheet lists what it did: its
    // `revision` is the call's sheet's, and a step that landed elsewhere is
    // taken back by naming those sheets (`docs/AGENT_EDITS.md`).
    let own_only = landed.len() == 1 && landed[0].0 == call_path;
    if !own_only && let Some(object) = answer.as_object_mut() {
        let sheets: Vec<Value> = landed
            .iter()
            .map(|(path, changed)| {
                json!({"document": file_name(path), "path": path.display().to_string(), "revision": revision_of(app, path), "changed": changed})
            })
            .collect();
        object.insert("sheets".to_string(), Value::Array(sheets));
    }
    (CommandOutcome::Answered(answer), Task::batch(ran.tasks))
}
