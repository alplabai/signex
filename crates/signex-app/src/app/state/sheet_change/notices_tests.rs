//! The edit tails' Messages lines (#2644): each is a catalog line that renders
//! in en-US byte-identical to the English the tail wrote before, and the
//! pseudo-locale words it differently.

use std::path::PathBuf;

use signex_engine::EngineError;
use signex_i18n::{I18n, LanguagePref, MessageId, ids};
use signex_i18n_ids::Translatable;

use super::notices::{engine_error_notice, locked_refusal, skip_notice};
use crate::app::core::event::{CoreEvent, NoticeBody, NoticeLevel};
use crate::app::{UndoGroupKind, UndoGroupSheets};

/// `text` in en-US, after checking it has an id and en-XA changes it.
fn en_us(text: &Translatable) -> String {
    assert!(text.id().is_some(), "a catalog line: {text:?}");
    let en = text.render(&I18n::en_us());
    let xa = I18n::new(&LanguagePref::Locale("en-XA".into()));
    assert_ne!(text.render(&xa), en, "the line follows the language");
    en
}

/// The level and en-US line of a text notice event.
fn line_of(event: CoreEvent) -> (NoticeLevel, String) {
    match event {
        CoreEvent::Notice(notice) => match &notice.body {
            NoticeBody::Text(text) => (notice.level, en_us(text)),
            NoticeBody::Engine(other) => panic!("not a text notice: {other:?}"),
        },
        CoreEvent::DocumentChanged(changed) => panic!("not a notice: {changed:?}"),
    }
}

#[test]
fn the_skip_line_reads_as_before() {
    let skipped: Vec<String> = ["R1", "C2"].map(String::from).to_vec();
    let (level, line) = line_of(skip_notice(&skipped).unwrap());
    assert_eq!(level, NoticeLevel::Warning);
    assert_eq!(
        line,
        "Skipped 2 locked object(s): R1, C2; unlock in Properties to change them"
    );
    assert!(skip_notice(&[]).is_none());
    // The count is text, never grouped.
    let many: Vec<String> = (0..1_000).map(|i| format!("R{i}")).collect();
    let (_, line) = line_of(skip_notice(&many).unwrap());
    assert!(
        line.starts_with("Skipped 1000 locked object(s): R0, R1"),
        "{line}"
    );
}

#[test]
fn each_engine_failure_line_reads_as_before() {
    let table: [(MessageId, &str); 5] = [
        (
            ids::NOTICE_EDIT_COMMAND_EXECUTION_FAILED,
            "Engine command execution failed",
        ),
        (ids::NOTICE_EDIT_COMMAND_FAILED, "Engine command failed"),
        (ids::NOTICE_EDIT_UNDO_FAILED, "Engine undo failed"),
        (ids::NOTICE_EDIT_REDO_FAILED, "Engine redo failed"),
        (
            ids::NOTICE_EDIT_GROUP_STEP_FAILED,
            "Engine group step failed",
        ),
    ];
    for (id, context) in table {
        let (level, line) = line_of(engine_error_notice(id, EngineError::MissingPath));
        assert_eq!(level, NoticeLevel::Error);
        let error = anyhow::Error::new(EngineError::MissingPath);
        assert_eq!(line, format!("{context}: {error:#}"));
    }
}

#[test]
fn the_locked_refusal_reads_as_the_engine_error() {
    for references in [vec!["R1"], vec!["R1", "C2"]] {
        let references: Vec<String> = references.into_iter().map(String::from).collect();
        let line = en_us(&locked_refusal(&references, "moved"));
        let error = EngineError::ItemLocked {
            references,
            action: "moved",
        };
        assert_eq!(line, error.to_string());
    }
    assert_eq!(
        en_us(&locked_refusal(&["R1".to_string()], "moved")),
        "R1 is locked and cannot be moved; unlock it in Properties first"
    );
}

#[test]
fn each_group_refusal_reads_as_before() {
    let sheets = vec![PathBuf::from("/w/a.snxsch"), PathBuf::from("/w/b.snxsch")];
    let recorded = UndoGroupSheets {
        kind: UndoGroupKind::Annotate,
        dropped: sheets.clone(),
        ..Default::default()
    };
    let table = [
        (
            recorded.redo_dropped(),
            "Cannot redo the project-wide annotate: a.snxsch, b.snxsch went back to its saved \
             state. Run Annotate again.",
        ),
        (
            recorded.blocked(true, &sheets),
            "Cannot undo the project-wide annotate: a.snxsch, b.snxsch has a later step (an edit \
             or a net-class pick). Undo the newer edit on a.snxsch, b.snxsch first.",
        ),
        (
            recorded.blocked(false, &sheets),
            "Cannot redo the project-wide annotate: a.snxsch, b.snxsch was edited after the undo, \
             so the annotate can't be redone across sheets.",
        ),
        (
            recorded.out_of_reach(true, &sheets),
            "Cannot undo the project-wide annotate: a.snxsch, b.snxsch was saved and closed, or \
             no longer holds this step, so no sheet was changed.",
        ),
        (
            recorded.out_of_reach(false, &sheets),
            "Cannot redo the project-wide annotate: a.snxsch, b.snxsch was saved and closed, or \
             no longer holds this step, so no sheet was changed.",
        ),
    ];
    for (text, line) in table {
        assert_eq!(en_us(&text), line);
    }
}
