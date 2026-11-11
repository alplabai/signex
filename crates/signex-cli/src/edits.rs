//! The checks a headless run and an attached run both make before a command
//! goes anywhere: each edit of an `apply_edits` call authorized as a command of
//! its own, and every `document` operand resolved by the app and confined.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_app::headless::{
    edit_list_text, parse_documents, parse_edit_list, parse_expect_revisions,
};
use signex_app::keymap::OperandKind;

use crate::capability::capability_of;
use crate::policy::{Approved, Policy};
use crate::protocol::Request;

/// The files the `document` operands of `approved` name, resolved by the app
/// (`resolve`: an id or a path to the file it is held as, or the app's reason)
/// and each checked like a path operand (inside the allowed folders, no link
/// out, within the size cap). An operand that names no document is refused.
/// With `hide_path` a refusal does not name the file the app resolved: an
/// attached run asked about an id of another open project must not learn where
/// that project's file is.
pub(crate) fn named_document_paths(
    policy: &Policy,
    roots: &[PathBuf],
    approved: &Approved,
    hide_path: bool,
    mut resolve: impl FnMut(&str) -> Result<PathBuf, String>,
) -> Result<Vec<PathBuf>, String> {
    let writes = capability_of(approved.meta).writes_files();
    let mut paths = Vec::new();
    let mut named: Vec<(&str, String)> = Vec::new();
    for (name, text) in &approved.operands {
        let is_document = approved
            .meta
            .operands
            .iter()
            .any(|operand| operand.name == *name && operand.kind == OperandKind::Document);
        if is_document {
            named.push((name, text.clone()));
        } else if name == "expect_revisions" && approved.command.as_str() == "apply_edits" {
            // The keys of a cross-sheet call's revisions name documents too.
            let keys = parse_expect_revisions(text)?;
            named.extend(
                keys.into_iter()
                    .map(|(document, _)| (name.as_str(), document)),
            );
        } else if name == "documents" && approved.command.as_str() == "revert" {
            // A revert moves every sheet of its group, so each is named and
            // confined like the `document` operand (#2178).
            let list = parse_documents(text)?;
            named.extend(list.into_iter().map(|document| (name.as_str(), document)));
        }
    }
    for (name, text) in &named {
        let path = resolve(text).map_err(|why| format!("operand `{name}`: {why}"))?;
        policy.check_resolved(&path, roots, writes).map_err(|why| {
            if hide_path {
                format!(
                    "operand `{name}`: the document it names is not inside the folders \
                         this run may use (--allow-path adds one)"
                )
            } else {
                format!("operand `{name}`: {why}")
            }
        })?;
        paths.push(path);
    }
    Ok(paths)
}

/// Keep, in the `sheets` list of an `apply_edits` or `revert` answer, only the
/// entries for documents this call named itself (`named`, already confined),
/// compared by full resolved path (`path`, as the app holds the sheet): the
/// answer never lists a sheet outside the run's folders, whatever the app says,
/// and a same-named file in another folder is not one the call named. Fail
/// closed: an entry with no `path`, or none named, is dropped.
pub(crate) fn scrub_sheets(data: &mut Value, named: &[PathBuf]) {
    let Some(Value::Array(sheets)) = data.get_mut("sheets") else {
        return;
    };
    sheets.retain(|entry| {
        entry["path"]
            .as_str()
            .is_some_and(|path| named.iter().any(|named| named == Path::new(path)))
    });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;
    use crate::confine::canonical;
    use crate::input::Limits;
    use crate::protocol::request_from_args;
    use serde_json::json;

    fn policy() -> Policy {
        Policy {
            capabilities: Capability::DEFAULT.into_iter().collect(),
            allow_paths: Vec::new(),
            limits: Limits::default(),
        }
    }

    /// A revert as the run approves it, naming `documents` besides `document`.
    fn revert(policy: &Policy, roots: &[PathBuf], documents: &[&Path]) -> Approved {
        let list = json!(
            documents
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
        );
        let args: Vec<String> = vec![
            format!("document={}", documents[0].display()),
            "group=1".into(),
            format!("documents={list}"),
        ];
        let request = request_from_args("revert", &args).unwrap();
        policy
            .authorize(&request, &roots[0], roots)
            .expect("the request is well formed")
    }

    /// #2178: a revert moves every sheet of its group, so each entry of
    /// `documents` is confined like the `document` operand: a run whose folders
    /// exclude sheet B cannot name it, and so cannot revert a group spanning it.
    #[test]
    fn every_sheet_a_revert_names_is_confined_to_the_runs_folders() {
        let (inside, outside) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let root = canonical(inside.path()).unwrap();
        let a = root.join("a.snxsch");
        let b = canonical(outside.path()).unwrap().join("b.snxsch");
        for file in [&a, &b] {
            std::fs::write(file, "x").unwrap();
        }
        let roots = vec![root];
        let policy = policy();
        let resolve = |text: &str| Ok(PathBuf::from(text));

        let both = revert(&policy, &roots, &[&a, &b]);
        let refused = named_document_paths(&policy, &roots, &both, true, resolve).unwrap_err();
        assert!(refused.contains("operand `documents`"), "{refused}");
        assert!(!refused.contains("b.snxsch"), "{refused}");

        let only_a = revert(&policy, &roots, &[&a]);
        let named = named_document_paths(&policy, &roots, &only_a, true, resolve).unwrap();
        assert_eq!(named, [a.clone(), a]);
    }

    /// The answer never lists a sheet the call did not name itself.
    #[test]
    fn the_sheets_of_an_answer_keep_only_the_documents_the_call_named() {
        let named = [PathBuf::from("/w/p/a.snxsch")];
        let mut data = json!({"reverted": true, "sheets": [
            {"document": "a.snxsch", "path": "/w/p/a.snxsch", "revision": 3},
            {"document": "b.snxsch", "path": "/w/q/b.snxsch", "revision": 9},
            // The same file name in a folder the call never named.
            {"document": "a.snxsch", "path": "/w/other/a.snxsch", "revision": 5},
            {"document": "a.snxsch", "revision": 1},
        ]});
        scrub_sheets(&mut data, &named);
        assert_eq!(
            data["sheets"],
            json!([{"document": "a.snxsch", "path": "/w/p/a.snxsch", "revision": 3}])
        );
        let mut none = json!({"changed": false});
        scrub_sheets(&mut none, &named);
        assert_eq!(none, json!({"changed": false}));
    }
}
