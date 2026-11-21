//! Footprint sketch updates — the parameters table (mockup row 189).
//!
//! Typing an expression goes the Properties list's way (one undo step per
//! keystroke, recorded by the router). Add, Rename, Delete and Make
//! parameter work on a copy of the sketch through
//! `signex_sketch::parameter_refs`, then put the copy in as one undo step,
//! re-solved; a refused edit changes nothing and records nothing.

use signex_sketch::SketchData;
use signex_sketch::mirrored_angle::parameter_from_mirrored_angle;
use signex_sketch::parameter_refs::{
    DeleteParameterError, DeletePolicy, delete_parameter, parameter_from_dimension,
    rename_parameter, resolve_rows, unique_name, used_by,
};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::parameters_table::SketchParametersMsg;
use crate::library::editor::footprint::parameters_table::update as table;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;
use crate::library::messages::FootprintEditorMsg;

/// What Add parameter names a new row, and the value it starts with.
const NEW_PARAMETER_STEM: &str = "param";
const NEW_PARAMETER_VALUE: &str = "1mm";

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    msg: SketchParametersMsg,
) {
    match msg {
        SketchParametersMsg::TableToggled => table::toggle(&mut editor.state.parameters_table),
        SketchParametersMsg::TableClosed => editor.state.parameters_table = None,
        SketchParametersMsg::ExpressionEdited { name, expr } => edit_expression(editor, name, expr),
        SketchParametersMsg::AddClicked => add(editor),
        SketchParametersMsg::RowMenuToggled { name } => {
            table::toggle_menu(&mut editor.state.parameters_table, name)
        }
        SketchParametersMsg::RenameStarted { name } => {
            table::start_rename(&mut editor.state.parameters_table, name)
        }
        SketchParametersMsg::RenameDraftChanged(draft) => {
            table::edit_rename(&mut editor.state.parameters_table, draft)
        }
        SketchParametersMsg::RenameSubmitted => rename(editor),
        SketchParametersMsg::RenameCancelled => {
            if let Some(t) = editor.state.parameters_table.as_deref_mut() {
                t.rename = None;
            }
        }
        SketchParametersMsg::DeleteRequested { name } => request_delete(editor, name),
        SketchParametersMsg::DeleteConfirmed => confirm_delete(editor),
        SketchParametersMsg::DeleteCancelled => {
            if let Some(t) = editor.state.parameters_table.as_deref_mut() {
                t.delete_prompt = None;
            }
        }
        SketchParametersMsg::ParameterFromConstraintRequested { constraint } => {
            make_parameter(editor, constraint)
        }
    }
    remember_values(editor);
}

/// The existing parameter edit, with the row focused for the autocomplete.
fn edit_expression(editor: &mut FootprintDocument, name: String, expr: String) {
    if let Some(t) = editor.state.parameters_table.as_deref_mut() {
        t.focus = Some(name.clone());
        t.menu = None;
    }
    super::constraints::apply(
        editor,
        FootprintEditorMsg::SketchEditParameter { name, expr },
    );
}

/// Add a row under the first free `param_N` name, focused.
fn add(editor: &mut FootprintDocument) {
    let mut sketch = current_sketch(editor);
    let name = unique_name(&sketch.parameters, NEW_PARAMETER_STEM);
    sketch.parameters.insert(name.clone(), NEW_PARAMETER_VALUE);
    commit(editor, sketch);
    table::open_focused(&mut editor.state.parameters_table, &name);
}

fn rename(editor: &mut FootprintDocument) {
    let Some(draft) = editor
        .state
        .parameters_table
        .as_deref()
        .and_then(|t| t.rename.clone())
    else {
        return;
    };
    let new = draft.draft.trim().to_string();
    let mut sketch = current_sketch(editor);
    if let Err(error) = rename_parameter(&mut sketch, &draft.from, &new) {
        if let Some(rename) = editor
            .state
            .parameters_table
            .as_deref_mut()
            .and_then(|t| t.rename.as_mut())
        {
            rename.error = Some(error);
        }
        return;
    }
    if new != draft.from {
        commit(editor, sketch);
    }
    table::open_focused(&mut editor.state.parameters_table, &new);
}

/// Delete an unused parameter at once; ask first for a used one.
fn request_delete(editor: &mut FootprintDocument, name: String) {
    let sketch = current_sketch(editor);
    if used_by(&sketch, &name).is_empty() {
        delete(editor, &name, DeletePolicy::Refuse);
        return;
    }
    if let Some(t) = editor.state.parameters_table.as_deref_mut() {
        t.menu = None;
        t.rename = None;
        t.delete_prompt = Some(name);
    }
}

/// "Replace with value": inline the value into every use, then delete, as
/// one undo step.
fn confirm_delete(editor: &mut FootprintDocument) {
    let Some(name) = editor
        .state
        .parameters_table
        .as_deref_mut()
        .and_then(|t| t.delete_prompt.take())
    else {
        return;
    };
    delete(editor, &name, DeletePolicy::Inline);
}

fn delete(editor: &mut FootprintDocument, name: &str, policy: DeletePolicy) {
    let mut sketch = current_sketch(editor);
    match delete_parameter(&mut sketch, name, policy) {
        Ok(_) => commit(editor, sketch),
        // The prompt names the uses; a refusal here means the sketch
        // changed under it, and nothing is deleted.
        Err(DeleteParameterError::InUse(_) | DeleteParameterError::Missing(_)) => {}
    }
    if let Some(t) = editor.state.parameters_table.as_deref_mut() {
        t.menu = None;
        if t.focus.as_deref() == Some(name) {
            t.focus = None;
        }
        t.last_good.remove(name);
    }
}

/// Make parameter on a dimension: a parameter holds its value and the
/// dimension reads it, one undo step; the table opens on that row. On a
/// board sketch a literal angle's parameter holds the value the user reads.
fn make_parameter(editor: &mut FootprintDocument, constraint: signex_sketch::id::ConstraintId) {
    let mut sketch = current_sketch(editor);
    let mirrored = editor
        .state
        .board_backdrop
        .is_some()
        .then(|| parameter_from_mirrored_angle(&mut sketch, constraint))
        .flatten();
    let made = match mirrored {
        Some(name) => Ok(name),
        None => parameter_from_dimension(&mut sketch, constraint),
    };
    match made {
        Ok(name) => {
            if editor.primitive().sketch.as_ref() != Some(&sketch) {
                commit(editor, sketch);
            }
            table::open_focused(&mut editor.state.parameters_table, &name);
        }
        Err(error) => crate::diagnostics::log_warning(format!("Make parameter: {error}")),
    }
}

/// The editor's sketch, or an empty one, as a copy to edit.
fn current_sketch(editor: &FootprintDocument) -> SketchData {
    editor.primitive().sketch.clone().unwrap_or_default()
}

/// Put `sketch` in as one undo step and re-solve it. A rejected solve
/// reverts it and records nothing.
fn commit(editor: &mut FootprintDocument, sketch: SketchData) {
    editor.record_edit(None, |ed| {
        let before = ed.primitive().sketch.clone();
        ed.with_parts(|state, primitive| {
            let result = mutate_solve_or_revert(state, primitive, |fp| fp.sketch = Some(sketch));
            if let Err(error) = result {
                report_solve_failure(state, &error);
            }
        });
        ed.canvas_cache.clear();
        let changed = ed.primitive().sketch != before;
        if changed {
            ed.mark_dirty();
        }
        changed
    });
}

/// Keep the value of every row that resolves, for the rows in error.
fn remember_values(editor: &mut FootprintDocument) {
    let rows = editor
        .primitive()
        .sketch
        .as_ref()
        .map(|s| resolve_rows(&s.parameters));
    if let (Some(t), Some(rows)) = (editor.state.parameters_table.as_deref_mut(), rows) {
        t.remember(&rows);
    }
}
