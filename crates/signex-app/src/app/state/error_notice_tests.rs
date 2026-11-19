//! #2058: the error notice keeps catalog ids and arguments, and the text is
//! rendered with the language in force, so an open card follows a switch.
//! Graphite m11 / GG-08: the severity of each heading, the system block, the
//! Copy text and the Save As… choice of File Changed on Disk.

use signex_i18n::RenderTranslatable;
use std::path::PathBuf;

use signex_i18n::{I18n, LanguagePref, Translatable, UserText};

use super::{ErrorNotice, NoticeSeverity, disk_changed_choices, untranslated, verbatim};
use crate::app::{Message, OverlayMsg, Signex, UiMsg};
use crate::persist::DiskDrift;

fn switch(app: &mut Signex, tag: &str) {
    let pref = LanguagePref::Locale(tag.to_owned());
    let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
}

fn shown(app: &Signex) -> (String, String) {
    let notice = app.document_state.error_notice.as_ref().expect("a notice");
    let i18n = &app.ui_state.i18n;
    (notice.title_text(i18n), notice.detail_text(i18n))
}

#[test]
fn an_open_notice_follows_a_language_switch() {
    let (mut app, _task) = Signex::new();
    switch(&mut app, "en-US");
    app.document_state.error_notice = Some(ErrorNotice::read_only("Basic Parts"));
    let english = (
        "Library Is Read-Only".to_owned(),
        "The library \"Basic Parts\" ships with Signex and is not editable.".to_owned(),
    );
    assert_eq!(shown(&app), english);

    switch(&mut app, "en-XA");
    let (title, detail) = shown(&app);
    assert_ne!(title, english.0, "the heading is rendered in en-XA");
    assert_ne!(detail, english.1, "the detail is rendered in en-XA");
    assert!(
        detail.contains("Basic Parts"),
        "the argument is kept: {detail}"
    );

    switch(&mut app, "en-US");
    assert_eq!(shown(&app), english);
}

#[test]
fn a_verbatim_detail_is_shown_as_it_is_in_every_language() {
    let (mut app, _task) = Signex::new();
    let line = "Permission denied (os error 13)";
    app.document_state.error_notice = Some(ErrorNotice::save(verbatim(line)));
    switch(&mut app, "en-XA");
    let (title, detail) = shown(&app);
    assert_ne!(title, "Save Failed");
    assert_eq!(detail, line);
}

/// The disk-changed card's English is what the code built before the
/// catalog. Since m11 the first paragraph is the sentence and the second is
/// the choice list (choice, effect), the same words; Save As… is new (GG-08).
#[test]
fn the_disk_changed_cards_keep_their_english() {
    let path = PathBuf::from("/w/proj/top.snxsch");
    let en = |drift| {
        let notice = ErrorNotice::disk_changed(path.clone(), drift);
        (notice.title.render_en_us(), notice.detail.render_en_us())
    };
    assert_eq!(
        en(DiskDrift::Changed),
        (
            "File Changed on Disk".to_owned(),
            "top.snxsch changed on disk since it was opened or last saved. Saving now would \
             discard that change."
                .to_owned()
        )
    );
    let choices = |save_as| -> Vec<(String, String)> {
        disk_changed_choices(save_as)
            .into_iter()
            .map(|(label, effect)| {
                (
                    Translatable::new(label).render_en_us(),
                    Translatable::new(effect).render_en_us(),
                )
            })
            .collect()
    };
    let pair = |label: &str, effect: &str| (label.to_owned(), effect.to_owned());
    assert_eq!(
        choices(true),
        vec![
            pair(
                "Save As\u{2026}",
                "writes this document to a new file and leaves the changed file as it is."
            ),
            pair("Overwrite", "replaces the file with this document."),
            pair(
                "Reload from disk",
                "loads the file and discards the unsaved edits in this document."
            ),
        ]
    );
    // Without Save As (a board), the list reads as the old second paragraph.
    let old_paragraph = choices(false)
        .into_iter()
        .map(|(label, effect)| format!("{label} {effect}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        old_paragraph,
        "Overwrite replaces the file with this document. Reload from disk loads the file and \
         discards the unsaved edits in this document."
    );
    assert_eq!(
        en(DiskDrift::Missing),
        (
            "File Missing on Disk".to_owned(),
            "top.snxsch is missing on disk (deleted or moved). Re-create writes this document \
             to /w/proj/top.snxsch."
                .to_owned()
        )
    );
    assert_eq!(
        en(DiskDrift::FolderMissing),
        (
            "Folder Missing on Disk".to_owned(),
            "top.snxsch cannot be saved: its folder is gone (moved or deleted). Nothing was \
             written. Put the folder back at /w/proj, then save again."
                .to_owned()
        )
    );
}

/// m11: red for a failure or refusal, amber for the cards that ask or warn.
#[test]
fn the_heading_decides_the_severity() {
    let sheet = || PathBuf::from("/w/proj/top.snxsch");
    let detail = || verbatim("x");
    for (notice, severity) in [
        (ErrorNotice::save(detail()), NoticeSeverity::Error),
        (ErrorNotice::export(detail()), NoticeSeverity::Error),
        (ErrorNotice::open(detail()), NoticeSeverity::Error),
        (ErrorNotice::annotate(detail()), NoticeSeverity::Error),
        (ErrorNotice::delete(detail()), NoticeSeverity::Error),
        (ErrorNotice::import(detail()), NoticeSeverity::Error),
        (
            ErrorNotice::disk_changed(sheet(), DiskDrift::Changed),
            NoticeSeverity::Warn,
        ),
        (
            ErrorNotice::disk_changed(sheet(), DiskDrift::Missing),
            NoticeSeverity::Warn,
        ),
        (
            ErrorNotice::disk_changed(sheet(), DiskDrift::FolderMissing),
            NoticeSeverity::Warn,
        ),
        (ErrorNotice::import_advisory(detail()), NoticeSeverity::Warn),
        (ErrorNotice::export_advisory(detail()), NoticeSeverity::Warn),
    ] {
        assert_eq!(
            notice.severity(),
            severity,
            "{}",
            notice.title.render_en_us()
        );
    }
}

/// Copy puts the heading, the sentence and the system block on the
/// clipboard, a blank line apart; a card without a system part copies the
/// first two.
#[test]
fn copy_text_is_the_heading_the_sentence_and_the_system_block() {
    let en = I18n::en_us();
    let notice = ErrorNotice::save(untranslated("Failed to save active document"))
        .with_system("writing C:\\work\\power.snxsch: Access is denied. (os error 5)");
    assert_eq!(
        notice.copy_text(&en),
        "Save Failed\n\nFailed to save active document\n\nwriting C:\\work\\power.snxsch: \
         Access is denied. (os error 5)"
    );
    let plain = ErrorNotice::save(verbatim("disk full"));
    assert_eq!(plain.copy_text(&en), "Save Failed\n\ndisk full");
    assert_eq!(
        ErrorNotice::save(verbatim("x")).with_system("  ").system,
        None,
        "a blank system part shows no block"
    );
}

/// A failed save names the operation in the sentence and keeps the error
/// chain (path and OS error) for the system block.
#[test]
fn a_failed_save_puts_the_error_chain_in_the_system_block() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    let error = anyhow::anyhow!("Access is denied. (os error 5)").context("writing /w/a.snxsch");
    app.report_save_failure("Failed to save active document", &error);
    let notice = app.document_state.error_notice.as_ref().expect("a card");
    assert_eq!(notice.title.render_en_us(), "Save Failed");
    assert_eq!(
        notice.detail.render_en_us(),
        "Failed to save active document"
    );
    assert_eq!(
        notice.system.as_ref().map(UserText::as_str),
        Some("writing /w/a.snxsch: Access is denied. (os error 5)")
    );
    // What leaves the screen (a signex-cli reason) is the one line the card
    // read before m11.
    assert_eq!(
        notice.detail_line_en_us(),
        "Failed to save active document: writing /w/a.snxsch: Access is denied. (os error 5)"
    );
}

/// GG-08: File Changed on Disk offers Save As… for a sheet, which has one,
/// and not for a board, nor on the other disk cards or a plain card.
#[test]
fn save_as_is_offered_for_a_changed_sheet_and_not_for_a_board() {
    let sheet = PathBuf::from("/w/proj/top.snxsch");
    let board = PathBuf::from("/w/proj/top.snxpcb");
    assert!(ErrorNotice::disk_changed(sheet.clone(), DiskDrift::Changed).offers_save_as());
    assert!(!ErrorNotice::disk_changed(board, DiskDrift::Changed).offers_save_as());
    assert!(!ErrorNotice::disk_changed(sheet.clone(), DiskDrift::Missing).offers_save_as());
    assert!(!ErrorNotice::disk_changed(sheet, DiskDrift::FolderMissing).offers_save_as());
    assert!(!ErrorNotice::save(verbatim("x")).offers_save_as());
}

fn conflict_fixture(dir: &std::path::Path) -> (Signex, PathBuf, PathBuf) {
    let sheet = || signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    let mut app =
        crate::app::handlers::menu::export::tests::app_workspace(&dir.to_string_lossy(), &[]);
    let conflicted = dir.join("conflict.snxsch");
    std::fs::write(&conflicted, b"their version").expect("seed");
    app.open_schematic_tab(conflicted.clone(), "conflict".into(), sheet());
    let other = dir.join("other.snxsch");
    app.open_schematic_tab(other.clone(), "other".into(), sheet());
    app.document_state.mark_document_dirty(&conflicted);
    (app, conflicted, other)
}

fn active_path(app: &Signex) -> PathBuf {
    app.document_state.tabs[app.document_state.active_tab]
        .path
        .clone()
}

/// GG-08: Save As… brings the conflicted sheet to the front, closes the
/// card and opens the Save As dialog for it. Nothing is written and the
/// sheet stays modified until that Save As lands.
#[test]
fn save_as_on_the_conflict_card_opens_save_as_for_the_conflicted_sheet() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, conflicted, other) = conflict_fixture(dir.path());
    assert_eq!(active_path(&app), other, "another tab is in front");
    app.document_state.error_notice = Some(ErrorNotice::disk_changed(
        conflicted.clone(),
        DiskDrift::Changed,
    ));

    let task = app.handle_disk_conflict_save_as();

    assert!(app.document_state.error_notice.is_none(), "the card closed");
    assert!(
        iced_runtime::task::into_stream(task).is_some(),
        "the Save As dialog is asked for"
    );
    assert_eq!(
        active_path(&app),
        conflicted,
        "the conflicted sheet is shown"
    );
    assert_eq!(
        std::fs::read(&conflicted).expect("read"),
        b"their version",
        "the changed file is left as it is"
    );
    assert!(app.document_state.is_document_dirty(&conflicted));
}

/// The card stays when Save As cannot run: a board has none, and a sheet
/// that is not a tab of the main window cannot be made the active one.
#[test]
fn save_as_keeps_the_card_when_it_cannot_run() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _conflicted, other) = conflict_fixture(dir.path());
    for path in [
        dir.path().join("board.snxpcb"),
        dir.path().join("gone.snxsch"),
    ] {
        let card = ErrorNotice::disk_changed(path, DiskDrift::Changed);
        app.document_state.error_notice = Some(card.clone());
        let _task = app.update(Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen));
        assert_eq!(app.document_state.error_notice.as_ref(), Some(&card));
        assert_eq!(active_path(&app), other, "no tab was switched");
    }
}
