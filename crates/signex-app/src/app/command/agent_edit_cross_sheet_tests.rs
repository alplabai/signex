//! #2178: one `apply_edits` call across the sheets of one project is one undo
//! group on all of them, all or nothing, with a revision expected per sheet.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::project_name_tests::{child, project_app};
use super::{CommandArgs, CommandOutcome};
use crate::app::Signex;
use crate::app::handlers::editing_commands::net_color_driver as fixture;
use crate::keymap::AppCommandId;

pub(super) fn run(app: &mut Signex, name: &str, operands: &[String]) -> CommandOutcome {
    let command = AppCommandId::new(name).expect("a valid id");
    let refs: Vec<&str> = operands.iter().map(String::as_str).collect();
    let args = CommandArgs::parse_operands(&command, &refs).expect("operands parse");
    app.run_operands(&command, &args).0
}

/// The project of `project_name_tests` (top and child sheets) with both
/// sheets open, top the active tab.
pub(super) fn two_sheets(dir: &tempfile::TempDir) -> Signex {
    let mut app = project_app(dir);
    app.open_or_focus_sheet_path(dir.path().join("child.snxsch"));
    app.open_or_focus_sheet_path(dir.path().join("top.snxsch"));
    app
}

pub(super) fn top(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("top.snxsch")
}

pub(super) fn kid(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("child.snxsch")
}

fn revision(app: &Signex, path: &Path) -> u64 {
    app.document_state.engines[path].revision()
}

fn quoted(path: &Path) -> String {
    path.display().to_string()
}

/// What a sheet is, for "nothing changed": revision, place on the timeline,
/// whether it is dirty, and the junctions it holds.
fn state(app: &Signex, path: &Path) -> (u64, usize, bool, usize) {
    let engine = &app.document_state.engines[path];
    (
        engine.revision(),
        engine.undo_position(),
        app.document_state.is_document_dirty(path),
        engine.document().junctions.len(),
    )
}

fn junctions(app: &Signex, path: &Path) -> usize {
    app.document_state.engines[path].document().junctions.len()
}

/// `apply_edits` on `top` with `others` as `expect_revisions`.
fn apply_with(
    app: &mut Signex,
    dir: &tempfile::TempDir,
    expect_revisions: Option<Value>,
    edits: Value,
) -> CommandOutcome {
    let mut operands = vec![
        format!("document={}", top(dir).display()),
        format!("expect_revision={}", revision(app, &top(dir))),
        format!("edits={edits}"),
    ];
    if let Some(others) = expect_revisions {
        operands.push(format!("expect_revisions={others}"));
    }
    run(app, "apply_edits", &operands)
}

/// `apply_edits` on `top` with `child` expected at its current revision.
pub(super) fn apply_both(
    app: &mut Signex,
    dir: &tempfile::TempDir,
    edits: Value,
) -> CommandOutcome {
    let others = json!({ quoted(&kid(dir)): revision(app, &kid(dir)) });
    apply_with(app, dir, Some(others), edits)
}

fn answered(outcome: CommandOutcome) -> Value {
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

fn refusal(outcome: CommandOutcome) -> String {
    match outcome {
        CommandOutcome::Disabled { reason } => reason,
        other => panic!("not refused: {other:?}"),
    }
}

/// A junction on `top` and one on `child`.
fn junction_on_both(dir: &tempfile::TempDir) -> Value {
    json!([
        {"id": "place_junction", "args": {"at": [20, 20]}},
        {"id": "place_junction", "args": {"at": [20, 20], "document": quoted(&kid(dir))}},
    ])
}

fn revert(app: &mut Signex, path: &Path, group: u64) -> CommandOutcome {
    run(
        app,
        "revert",
        &[
            format!("document={}", path.display()),
            format!("group={group}"),
        ],
    )
}

/// A revert that names every sheet of the group: the first as `document`, all
/// of them as `documents`.
fn revert_all(app: &mut Signex, paths: &[&Path], group: u64) -> CommandOutcome {
    let list: Vec<String> = paths.iter().map(|p| quoted(p)).collect();
    run(
        app,
        "revert",
        &[
            format!("document={}", paths[0].display()),
            format!("group={group}"),
            format!("documents={}", json!(list)),
        ],
    )
}

/// Edit 2, on the parent, reads the project (the cache of the list fills);
/// edit 3 puts the global name `Y` on the child's net, behind the sheet pin
/// `SIG`; edit 4, on the parent again, joins the net labelled `FREE` to that
/// net. The project check of edit 4 must see edit 3's `Y` on the child, so the
/// names it reports merging include it.
#[test]
fn an_edit_on_one_sheet_is_seen_by_the_name_check_of_a_later_edit_on_the_other() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let kid_path = quoted(&kid(&dir));
    let answer = answered(apply_both(
        &mut app,
        &dir,
        json!([
            {"id": "place_net_label", "args": {"at": [5.08, 40.64], "text": "FREE"}},
            {"id": "place_wire", "args": {"start": [5.08, 0], "end": [10.16, 0], "allow_merge": true}},
            {"id": "place_global_label", "args": {"at": [5.08, 0], "text": "Y", "document": kid_path, "allow_merge": true}},
            {"id": "place_wire", "args": {"start": [5.08, 40.64], "end": [5.08, 0], "allow_merge": true}},
        ]),
    ));
    let merged = answer["results"][3]["merged"].to_string();
    assert!(merged.contains("\"Y\""), "edit 4 did not see Y: {answer}");
}

/// The call names `top`, but every edit names `child`: the step lands on the
/// child alone. The answer says so (`sheets`, with the child's new revision;
/// `revision` stays the call's own sheet's), and the step is taken back by
/// naming the sheet that has it.
#[test]
fn a_call_whose_edits_all_name_another_sheet_lists_it_and_is_reverted_by_naming_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let top_before = state(&app, &top);
    let kid_at = revision(&app, &kid);
    let answer = answered(apply_with(
        &mut app,
        &dir,
        Some(json!({ quoted(&kid): kid_at })),
        json!([{"id": "place_junction", "args": {"at": [20, 20], "document": quoted(&kid)}}]),
    ));
    let group = answer["group"].as_u64().expect("a group");
    assert_eq!(answer["revision"], top_before.0, "the call's own sheet's");
    assert_eq!(state(&app, &top), top_before, "the top sheet got no step");
    let sheets = answer["sheets"].as_array().expect("a sheet list");
    assert_eq!(sheets.len(), 1, "{answer}");
    assert_eq!(sheets[0]["document"], "child.snxsch");
    assert_eq!(sheets[0]["revision"], revision(&app, &kid));
    assert_eq!(sheets[0]["changed"], true);
    assert_ne!(revision(&app, &kid), kid_at);

    // The sheet without the step cannot take it back; the one with it can.
    let reason = refusal(revert(&mut app, &top, group));
    assert!(reason.contains("no longer in the history"), "{reason}");
    let done = answered(revert(&mut app, &kid, group));
    assert_eq!(done["reverted"], true);
    assert_eq!(junctions(&app, &kid), 0);
    assert_eq!(state(&app, &top), top_before);
}

#[test]
fn a_list_across_two_sheets_is_one_group_and_one_ctrl_z_undoes_it_on_both() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let before = (state(&app, &top), state(&app, &kid));
    let answer = answered(apply_both(&mut app, &dir, junction_on_both(&dir)));
    assert_eq!(answer["changed"], true);
    let group = answer["group"].as_u64().expect("a group");
    let sheets = answer["sheets"].as_array().expect("a sheet list");
    assert_eq!(sheets.len(), 2, "{answer}");
    assert_eq!(answer["revision"], revision(&app, &top), "the call's own");
    for path in [&top, &kid] {
        let engine = &app.document_state.engines[path];
        assert_eq!(engine.next_undo_group().map(|g| g.get()), Some(group));
        assert_eq!(engine.undo_position(), before.0.1 + 1, "one step each");
        assert_eq!(junctions(&app, path), 1);
        assert!(app.document_state.is_document_dirty(path));
    }
    let record = &app.document_state.undo_groups[&signex_engine::UndoGroupId::new(group)];
    assert_eq!(record.live.len(), 2);

    // The person's Ctrl+Z on the active sheet takes the group on both.
    assert!(app.apply_engine_undo(false));
    assert_eq!((junctions(&app, &top), junctions(&app, &kid)), (0, 0));
    assert!(!app.document_state.is_document_dirty(&top));
    assert!(!app.document_state.is_document_dirty(&kid));
    // And Ctrl+Y brings it back on both.
    assert!(app.apply_engine_redo(false));
    assert_eq!((junctions(&app, &top), junctions(&app, &kid)), (1, 1));
}

#[test]
fn a_refusal_on_the_second_sheet_rolls_the_first_back() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let before = (state(&app, &top), state(&app, &kid));
    let reason = refusal(apply_both(
        &mut app,
        &dir,
        json!([
            {"id": "place_junction", "args": {"at": [20, 20]}},
            {"id": "place_wire", "args": {"start": [34, 34], "end": [36, 34], "document": quoted(&kid)}},
        ]),
    ));
    assert!(reason.contains("edit 2 (`place_wire`)"), "{reason}");
    assert!(reason.contains("Nothing was applied"), "{reason}");
    assert_eq!((state(&app, &top), state(&app, &kid)), before);
    assert!(app.document_state.undo_groups.is_empty());
}

#[test]
fn a_revision_is_expected_per_sheet_and_a_wrong_one_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let before = (state(&app, &top), state(&app, &kid));
    let kid_at = revision(&app, &kid);
    let edits = junction_on_both(&dir);

    let stale = apply_with(
        &mut app,
        &dir,
        Some(json!({ quoted(&kid): kid_at + 1 })),
        edits.clone(),
    );
    let reason = refusal(stale);
    assert!(reason.contains("child.snxsch"), "{reason}");
    assert!(reason.contains("changed since revision"), "{reason}");

    let missing = refusal(apply_with(&mut app, &dir, None, edits.clone()));
    assert!(missing.contains("`expect_revisions`"), "{missing}");
    assert!(missing.contains("child.snxsch"), "{missing}");

    let own = json!({ quoted(&kid): kid_at, quoted(&top): revision(&app, &top) });
    let named_twice = refusal(apply_with(&mut app, &dir, Some(own), edits));
    assert!(named_twice.contains("`expect_revision`"), "{named_twice}");

    let only_top = json!([{"id": "place_junction", "args": {"at": [20, 20]}}]);
    let untouched = refusal(apply_with(
        &mut app,
        &dir,
        Some(json!({ quoted(&kid): kid_at })),
        only_top,
    ));
    assert!(untouched.contains("no edit touches"), "{untouched}");

    assert_eq!((state(&app, &top), state(&app, &kid)), before);
}

#[test]
fn an_edit_naming_a_sheet_of_another_project_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    // A folder of its own, so the path is absolute on every OS.
    let elsewhere = tempfile::tempdir().unwrap();
    let other = elsewhere.path().join("other.snxsch");
    fixture::add_sheet_with(&mut app, &other, child());
    fixture::activate(&mut app, &top(&dir));
    let top = top(&dir);
    let before = state(&app, &top);
    let others = json!({ quoted(&other): revision(&app, &other) });
    let reason = refusal(apply_with(
        &mut app,
        &dir,
        Some(others),
        json!([{"id": "place_junction", "args": {"at": [20, 20], "document": quoted(&other)}}]),
    ));
    assert!(reason.contains("another project"), "{reason}");
    assert_eq!(state(&app, &top), before);
}

#[test]
fn a_read_only_second_sheet_refuses_the_whole_call() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let mut sheet = child();
    sheet.unknown.newer_minor_source = Some("snxsch/9.9".into());
    app.document_state
        .engines
        .insert(kid.clone(), signex_engine::Engine::new(sheet).unwrap());
    let before = (state(&app, &top), state(&app, &kid));
    let reason = refusal(apply_both(&mut app, &dir, junction_on_both(&dir)));
    assert!(
        reason.contains("read-only") || reason.contains("newer"),
        "{reason}"
    );
    assert_eq!((state(&app, &top), state(&app, &kid)), before);
}

#[test]
fn the_headless_undo_of_a_group_across_sheets_points_to_revert() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    answered(apply_both(&mut app, &dir, junction_on_both(&dir)));
    for path in [&top, &kid] {
        let reason = refusal(run(
            &mut app,
            "undo",
            &[format!("document={}", path.display())],
        ));
        assert!(reason.contains("`revert`"), "{reason}");
        assert_eq!(junctions(&app, path), 1, "nothing moved");
    }
}

#[test]
fn a_later_step_on_one_sheet_blocks_both_the_persons_undo_and_the_revert() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let group = answered(apply_both(&mut app, &dir, junction_on_both(&dir)))["group"]
        .as_u64()
        .unwrap();
    // A later agent step lands on the child only.
    let kid_at = revision(&app, &kid);
    let later = run(
        &mut app,
        "apply_edits",
        &[
            format!("document={}", kid.display()),
            format!("expect_revision={kid_at}"),
            format!(
                "edits={}",
                json!([{"id": "place_no_connect", "args": {"at": [30, 30]}}])
            ),
        ],
    );
    let later = answered(later)["group"].as_u64().unwrap();
    let held = (state(&app, &top), state(&app, &kid));

    // Ctrl+Z on the top sheet would revert the group underneath the child's
    // later step: refused, nothing moves.
    let mark = crate::diagnostics::newest_entry_id();
    assert!(!app.apply_engine_undo(false));
    assert_eq!((state(&app, &top), state(&app, &kid)), held);
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        said.iter()
            .any(|m| m.contains("Cannot undo the cross-sheet agent edit")
                && m.contains("child.snxsch")),
        "{said:?}"
    );
    // So is a revert, and the sheet that is free is not touched either.
    let reason = refusal(revert_all(&mut app, &[&top, &kid], group));
    assert!(reason.contains("child.snxsch"), "{reason}");
    assert!(reason.contains("later step"), "{reason}");
    assert_eq!((state(&app, &top), state(&app, &kid)), held);

    // Newest first: the later step, then the group on both sheets.
    let single = answered(revert(&mut app, &kid, later));
    assert_eq!(single["reverted"], true);
    assert!(
        single.get("sheets").is_none(),
        "a one-sheet step names none"
    );
    let done = answered(revert_all(&mut app, &[&top, &kid], group));
    assert_eq!(done["reverted"], true);
    assert_eq!(done["sheets"].as_array().map(Vec::len), Some(2), "{done}");
    assert_eq!((junctions(&app, &top), junctions(&app, &kid)), (0, 0));
    assert!(!app.document_state.is_document_dirty(&top));
    assert!(!app.document_state.is_document_dirty(&kid));
    assert!(app.document_state.undo_groups.is_empty());
    // A reverted step is gone: not on the redo stack of either sheet.
    assert!(!app.document_state.engines[&top].can_redo());
    assert!(!app.document_state.engines[&kid].can_redo());
}

#[test]
fn a_revert_is_refused_naming_a_member_sheet_that_is_not_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let group = answered(apply_both(&mut app, &dir, junction_on_both(&dir)))["group"]
        .as_u64()
        .unwrap();
    let engine = app.document_state.engines.remove(&kid).unwrap();
    let reason = refusal(revert_all(&mut app, &[&top, &kid], group));
    assert!(reason.contains("child.snxsch"), "{reason}");
    assert_eq!(junctions(&app, &top), 1, "nothing was reverted");
    app.document_state.engines.insert(kid.clone(), engine);
    assert_eq!(junctions(&app, &kid), 1);
}

/// A revert moves every sheet of its group, so it must name them all: naming
/// one is refused, and the reason counts the sheets left out without naming
/// them (the caller may not be allowed to know them).
#[test]
fn a_revert_naming_one_sheet_of_a_group_is_refused_without_naming_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let group = answered(apply_both(&mut app, &dir, junction_on_both(&dir)))["group"]
        .as_u64()
        .unwrap();
    let held = (state(&app, &top), state(&app, &kid));
    let reason = refusal(revert(&mut app, &top, group));
    assert!(reason.contains("1 not named"), "{reason}");
    assert!(
        !reason.contains("child"),
        "no other sheet is named: {reason}"
    );
    assert_eq!((state(&app, &top), state(&app, &kid)), held);
    // The other order too: the child alone does not name the top.
    let reason = refusal(revert(&mut app, &kid, group));
    assert!(!reason.contains("top"), "{reason}");
    assert_eq!((state(&app, &top), state(&app, &kid)), held);
}

/// Naming a sheet the group does not cover is refused too, with the count only.
#[test]
fn a_revert_naming_a_sheet_outside_the_group_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let top_at = revision(&app, &top);
    let applied = answered(run(
        &mut app,
        "apply_edits",
        &[
            format!("document={}", top.display()),
            format!("expect_revision={top_at}"),
            format!(
                "edits={}",
                json!([{"id": "place_junction", "args": {"at": [20, 20]}}])
            ),
        ],
    ));
    let group = applied["group"].as_u64().unwrap();
    let held = (state(&app, &top), state(&app, &kid));
    let reason = refusal(revert_all(&mut app, &[&top, &kid], group));
    assert!(reason.contains("1 named that are not in it"), "{reason}");
    assert_eq!((state(&app, &top), state(&app, &kid)), held);
    // Naming just the sheet it covers still works.
    assert_eq!(answered(revert(&mut app, &top, group))["reverted"], true);
}

/// Naming every sheet of the group takes it back on all of them, and the
/// answer lists them.
#[test]
fn a_revert_naming_every_sheet_takes_the_group_back_on_both() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = two_sheets(&dir);
    let (top, kid) = (top(&dir), kid(&dir));
    let group = answered(apply_both(&mut app, &dir, junction_on_both(&dir)))["group"]
        .as_u64()
        .unwrap();
    let done = answered(revert_all(&mut app, &[&kid, &top], group));
    assert_eq!(done["reverted"], true);
    assert_eq!(done["sheets"].as_array().map(Vec::len), Some(2), "{done}");
    assert_eq!((junctions(&app, &top), junctions(&app, &kid)), (0, 0));
}
