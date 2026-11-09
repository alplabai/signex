//! The checks a headless run and an attached run both make before a command
//! goes anywhere: each edit of an `apply_edits` call authorized as a command of
//! its own, and every `document` operand resolved by the app and confined.

use std::path::{Path, PathBuf};

use signex_app::headless::{edit_list_text, parse_edit_list};
use signex_app::keymap::OperandKind;

use crate::capability::capability_of;
use crate::policy::{Approved, Policy};
use crate::protocol::Request;

/// The files the `document` operands of `approved` name, resolved by the app
/// (`resolve`: an id or a path to the file it is held as, or the app's reason)
/// and each checked like a path operand (inside the allowed folders, no link
/// out, within the size cap). An operand that names no document is refused.
pub(crate) fn named_document_paths(
    policy: &Policy,
    roots: &[PathBuf],
    approved: &Approved,
    mut resolve: impl FnMut(&str) -> Result<PathBuf, String>,
) -> Result<Vec<PathBuf>, String> {
    let writes = capability_of(approved.meta).writes_files();
    let mut paths = Vec::new();
    for (name, text) in &approved.operands {
        let is_document = approved
            .meta
            .operands
            .iter()
            .any(|operand| operand.name == *name && operand.kind == OperandKind::Document);
        if !is_document {
            continue;
        }
        let path = resolve(text).map_err(|why| format!("operand `{name}`: {why}"))?;
        policy
            .check_resolved(&path, roots, writes)
            .map_err(|why| format!("operand `{name}`: {why}"))?;
        paths.push(path);
    }
    Ok(paths)
}

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
