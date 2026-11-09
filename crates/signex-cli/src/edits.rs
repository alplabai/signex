//! The edits of an `apply_edits` call, each authorized as a command of its
//! own: the one check a headless run and an attached run both make before the
//! call goes anywhere.

use std::path::{Path, PathBuf};

use signex_app::headless::{edit_list_text, parse_edit_list};

use crate::policy::{Approved, Policy};
use crate::protocol::Request;

/// Authorize every edit of `approved` (an `apply_edits` call; any other
/// command has none) like a command of its own (not `gui_only`, the `edit`
/// capability held, every path operand confined and still the same path), then
/// write the edit list back with the checked paths. `resolve` is the last
/// check of an edit, made on its authorized form: a headless run resolves the
/// `document` operands through its app there.
pub(crate) fn authorize_edits(
    policy: &Policy,
    base: &Path,
    roots: &[PathBuf],
    approved: &mut Approved,
    mut resolve: impl FnMut(&Approved) -> Result<(), String>,
) -> Result<(), String> {
    if approved.command.as_str() != "apply_edits" {
        return Ok(());
    }
    let Some(at) = approved
        .operands
        .iter()
        .position(|(name, _)| name == "edits")
    else {
        return Ok(());
    };
    let mut edits = parse_edit_list(&approved.operands[at].1)?;
    for (index, edit) in edits.iter_mut().enumerate() {
        let why = |reason: String| format!("edit {} (`{}`): {reason}", index + 1, edit.id);
        let request = Request {
            id: edit.id.clone(),
            operands: edit.operands.clone(),
        };
        let checked = policy.authorize(&request, base, roots).map_err(&why)?;
        policy.recheck(&checked, roots).map_err(&why)?;
        resolve(&checked).map_err(&why)?;
        edit.operands = checked.operands;
    }
    approved.operands[at].1 = edit_list_text(&edits);
    Ok(())
}
