//! #2058: the error notice keeps catalog ids and arguments, and the text is
//! rendered with the language in force, so an open card follows a switch.
//! Graphite mockup row 11: the severity of each heading, the system block, the
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
/// catalog. Since mockup row 11 the first paragraph is the sentence and the second is
/// the choice list (choice, effect), the same words; Save As… is new.
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

/// Mockup row 11: red for a failure or refusal, amber for the cards that ask or warn.
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
        "Save Failed\n\nFailed to save active document Your edits are still open and \
         unsaved.\n\nwriting C:\\work\\power.snxsch: Access is denied. (os error 5)"
    );
    let plain = ErrorNotice::save(verbatim("disk full"));
    assert_eq!(
        plain.copy_text(&en),
        "Save Failed\n\ndisk full Your edits are still open and unsaved."
    );
    assert_eq!(
        ErrorNotice::save(verbatim("x")).with_system("  ").system,
        None,
        "a blank system part shows no block"
    );
}

/// Mockup row 11: a Save Failed card says, after its detail, that nothing
/// was lost. The sentence comes from the catalog, Copy carries it, and the
/// one line that leaves the screen does not. Other cards add nothing.
#[test]
fn a_save_failed_card_says_the_edits_are_still_open() {
    let en = I18n::en_us();
    let notice = ErrorNotice::save(verbatim("power.snxsch could not be written."));
    assert_eq!(
        notice.note,
        Some(signex_i18n::ids::NOTICE_SAVE_FAILED_EDITS_KEPT)
    );
    let sentence = "power.snxsch could not be written. Your edits are still open and unsaved.";
    assert_eq!(notice.sentence_text(&en), sentence);
    assert_eq!(notice.copy_text(&en), format!("Save Failed\n\n{sentence}"));
    assert_eq!(
        notice.detail_line_en_us(),
        "power.snxsch could not be written."
    );
    for other in [
        ErrorNotice::export(verbatim("x")),
        ErrorNotice::create(verbatim("x")),
        ErrorNotice::disk_changed(PathBuf::from("/w/a.snxsch"), DiskDrift::Changed),
    ] {
        assert_eq!(other.note, None, "{:?}", other.title);
        assert_eq!(other.sentence_text(&en), other.detail_text(&en));
    }
}

/// A Save Failed card for a write that leaves no open edits (the global
/// library list) keeps the heading and claims nothing about unsaved edits.
#[test]
fn a_save_failed_card_with_no_open_edits_has_no_note() {
    let en = I18n::en_us();
    let notice = ErrorNotice::save_with_no_open_edits(verbatim("global_libraries.toml: denied"));
    assert_eq!(notice.title_text(&en), "Save Failed");
    assert_eq!(notice.note, None);
    assert_eq!(
        notice.copy_text(&en),
        "Save Failed\n\nglobal_libraries.toml: denied"
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
    // read before mockup row 11.
    assert_eq!(
        notice.detail_line_en_us(),
        "Failed to save active document: writing /w/a.snxsch: Access is denied. (os error 5)"
    );
}

/// File Changed on Disk offers Save As… for a sheet, which has one,
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
    for has in ["/w/lib/R.snxsym", "/w/lib/R0603.SNXFPT"] {
        assert!(
            ErrorNotice::disk_changed(PathBuf::from(has), DiskDrift::Changed).offers_save_as(),
            "{has}"
        );
    }
    for none in ["/w/proj/top.snxprj", "/w/lib/parts.snxlib", "/w/notes.txt"] {
        assert!(
            !ErrorNotice::disk_changed(PathBuf::from(none), DiskDrift::Changed).offers_save_as(),
            "{none}: a kind without Save As offers none"
        );
    }
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

/// Save As… on the card brings the conflicted sheet to the front, closes the
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

/// Copy on the card hands the clipboard write back as a Task and leaves the
/// card up; with no card there is nothing to copy and no work.
#[test]
fn copy_on_the_card_writes_the_clipboard_through_a_task() {
    let (mut app, _boot) = Signex::new();
    let card = ErrorNotice::create(verbatim("it failed"));
    app.document_state.error_notice = Some(card.clone());
    let task = app.update(Message::Overlay(OverlayMsg::ErrorNoticeCopyPressed));
    assert!(task.units() > 0, "the clipboard write is a Task");
    assert_eq!(app.document_state.error_notice.as_ref(), Some(&card));

    app.document_state.error_notice = None;
    let task = app.update(Message::Overlay(OverlayMsg::ErrorNoticeCopyPressed));
    assert_eq!(task.units(), 0, "no card, no work");
}

/// Answer the next Save As dialog with `path`, replacing a file that is there.
fn answer_save_as(app: &mut Signex, path: &std::path::Path) {
    let answers = crate::io_ports::HeadlessAnswers::default();
    answers.set("path", path.to_string_lossy().into_owned());
    answers.set("overwrite", "true");
    app.ui_state.io = crate::io_ports::IoPorts::headless(answers);
}

/// Data loss: Save As from the card onto the changed file itself, in
/// any spelling, writes nothing. Their version stays on disk byte for byte,
/// the sheet stays modified and the card comes back.
#[test]
fn save_as_from_the_card_onto_the_changed_file_leaves_their_version() {
    let _ = crate::diagnostics::init_logging();
    for spelling in [
        "conflict.snxsch",
        "./conflict.snxsch",
        "gone/../conflict.snxsch",
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        let (mut app, conflicted, _other) = conflict_fixture(dir.path());
        let card = ErrorNotice::disk_changed(conflicted.clone(), DiskDrift::Changed);
        app.document_state.error_notice = Some(card.clone());
        answer_save_as(&mut app, &dir.path().join(spelling));

        crate::app::path_ops::settle(
            &mut app,
            Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen),
        );

        assert_eq!(
            std::fs::read(&conflicted).expect("read"),
            b"their version",
            "{spelling}: their version stays"
        );
        assert_eq!(
            app.document_state.error_notice.as_ref(),
            Some(&card),
            "{spelling}: the card is back"
        );
        assert!(
            app.document_state.is_document_dirty(&conflicted),
            "{spelling}: the sheet stays modified"
        );
        assert_eq!(active_path(&app), conflicted, "{spelling}: no re-key");
    }

    // A symbol's Save As comes back through the same check.
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, _sheet, _other) = conflict_fixture(dir.path());
    let symbol = dir.path().join("part.snxsym");
    std::fs::write(&symbol, b"their version").expect("seed");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("A"));
    app.open_symbol_editor_tab(symbol.clone(), file);
    app.document_state.mark_document_dirty(&symbol);
    let card = ErrorNotice::disk_changed(symbol.clone(), DiskDrift::Changed);
    app.document_state.error_notice = Some(card.clone());
    answer_save_as(&mut app, &symbol);

    crate::app::path_ops::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen),
    );

    assert_eq!(
        std::fs::read(&symbol).expect("read"),
        b"their version",
        "symbol: their version stays"
    );
    assert_eq!(
        app.document_state.error_notice.as_ref(),
        Some(&card),
        "symbol: the card is back"
    );
    assert!(app.document_state.is_document_dirty(&symbol));
    assert_eq!(active_path(&app), symbol, "symbol: no re-key");
}

/// Data loss: Cancel on the card, then File ▸ Save As onto the sheet's
/// own file. That is a plain save and is compared like Ctrl+S, so
/// their version stays byte for byte and the card comes back.
#[test]
fn file_save_as_onto_the_changed_file_itself_asks_like_ctrl_s() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, conflicted, _other) = conflict_fixture(dir.path());
    // The sheet was read as "our version"; another program wrote theirs since.
    app.record_disk_baseline(&conflicted, crate::persist::ContentHash::of(b"our version"));
    assert!(app.activate_tab_for_path(&conflicted));
    assert!(
        app.document_state.error_notice.is_none(),
        "the card was cancelled"
    );

    crate::app::path_ops::settle(
        &mut app,
        Message::File(crate::app::FileMsg::SaveAs {
            from_path: conflicted.clone(),
            to_path: conflicted.clone(),
        }),
    );

    assert_eq!(
        std::fs::read(&conflicted).expect("read"),
        b"their version",
        "their version stays"
    );
    assert_eq!(
        app.document_state.error_notice.as_ref(),
        Some(&ErrorNotice::disk_changed(
            conflicted.clone(),
            DiskDrift::Changed
        )),
        "the card asks again"
    );
    assert!(app.document_state.is_document_dirty(&conflicted));
}

/// File ▸ Save As onto the sheet's own file after that file was deleted
/// writes it, as before: no Missing on Disk card (Caner, 2026-11-20).
#[test]
fn file_save_as_onto_its_deleted_own_file_writes_it() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, sheet, _other) = conflict_fixture(dir.path());
    app.record_disk_baseline(&sheet, crate::persist::ContentHash::of(b"our version"));
    std::fs::remove_file(&sheet).expect("delete");
    assert!(app.activate_tab_for_path(&sheet));

    crate::app::path_ops::settle(
        &mut app,
        Message::File(crate::app::FileMsg::SaveAs {
            from_path: sheet.clone(),
            to_path: sheet.clone(),
        }),
    );

    let text = std::fs::read_to_string(&sheet).expect("the file is written again");
    let written = signex_types::format::SnxSchematic::parse(&text)
        .expect("the open sheet, encoded")
        .sheet;
    let open = app.active_schematic().expect("the sheet is still open");
    assert_eq!(written.uuid, open.uuid, "the open sheet is what is written");
    assert_eq!(written.paper_size, "A4");
    assert_eq!(app.document_state.error_notice, None, "no card");
    assert!(!app.document_state.is_document_dirty(&sheet));
    assert_eq!(active_path(&app), sheet, "no re-key");
}

/// Save As from the card, to the end: Save As to a new file writes the
/// document there and nothing else. Their version stays, the tab names the
/// new file, the refused save's failure record is gone and the tab shows
/// no dirty dot, as the document on it is saved. A symbol does the same.
#[test]
fn save_as_from_the_card_to_a_new_file_moves_the_document_there() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut app, conflicted, _other) = conflict_fixture(dir.path());
    // A save that found the file changed records the failure and raises the card.
    app.handle_save_found_disk_drift(conflicted.clone(), DiskDrift::Changed);
    let saves = &app.document_state.document_saves;
    assert!(saves.failure(&conflicted).is_some(), "the refused save");
    assert!(app.document_state.error_notice.is_some(), "the card");
    let copy = dir.path().join("copy.snxsch");
    answer_save_as(&mut app, &copy);

    crate::app::path_ops::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen),
    );

    assert_eq!(
        std::fs::read(&conflicted).expect("read"),
        b"their version",
        "the changed file is untouched"
    );
    let written = std::fs::read(&copy).expect("the new file is written");
    assert_ne!(written, b"their version");
    assert_eq!(active_path(&app), copy, "the tab names the new file");
    assert!(
        app.document_state.tabs.iter().all(|t| t.path != conflicted),
        "no tab is left on the changed file"
    );
    let saves = &app.document_state.document_saves;
    assert_eq!(saves.failure(&conflicted), None, "the old failure is gone");
    assert_eq!(
        saves.failure(&copy),
        None,
        "and did not follow the document"
    );
    assert!(
        !app.document_state.is_document_dirty(&copy),
        "the saved tab shows no dirty dot"
    );
    assert!(!app.document_state.is_document_dirty(&conflicted));

    // A symbol's Save As lands the same way and clears the failure recorded
    // under the file it left.
    let symbol = dir.path().join("part.snxsym");
    std::fs::write(&symbol, b"their version").expect("seed");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("A"));
    app.open_symbol_editor_tab(symbol.clone(), file);
    app.document_state.mark_document_dirty(&symbol);
    app.handle_save_found_disk_drift(symbol.clone(), DiskDrift::Changed);
    let saves = &app.document_state.document_saves;
    assert!(saves.failure(&symbol).is_some(), "symbol: the refused save");
    assert_eq!(
        app.document_state.error_notice,
        Some(ErrorNotice::disk_changed(
            symbol.clone(),
            DiskDrift::Changed
        )),
        "symbol: the card"
    );
    let symbol_copy = dir.path().join("copy.snxsym");
    answer_save_as(&mut app, &symbol_copy);

    crate::app::path_ops::settle(
        &mut app,
        Message::Overlay(OverlayMsg::DiskConflictSaveAsChosen),
    );

    assert_eq!(
        std::fs::read(&symbol).expect("read"),
        b"their version",
        "symbol: the changed file is untouched"
    );
    let written = std::fs::read(&symbol_copy).expect("symbol: the new file is written");
    assert_ne!(written, b"their version");
    assert_eq!(
        active_path(&app),
        symbol_copy,
        "symbol: the tab names the new file"
    );
    let saves = &app.document_state.document_saves;
    assert_eq!(
        saves.failure(&symbol),
        None,
        "symbol: the old failure is gone"
    );
    assert_eq!(saves.failure(&symbol_copy), None);
    assert!(!app.document_state.is_document_dirty(&symbol_copy));
    assert!(!app.document_state.is_document_dirty(&symbol));
}
