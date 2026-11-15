//! #2058: the error notice keeps catalog ids and arguments, and the text is
//! rendered with the language in force, so an open card follows a switch.

use std::path::PathBuf;

use signex_i18n::LanguagePref;

use super::{ErrorNotice, verbatim};
use crate::app::{Message, Signex, UiMsg};
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
/// catalog, paragraph break included.
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
             discard that change.\n\nOverwrite replaces the file with this document. Reload \
             from disk loads the file and discards the unsaved edits in this document."
                .to_owned()
        )
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
