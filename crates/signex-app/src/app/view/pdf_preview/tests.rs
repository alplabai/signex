//! #2058: the Export PDF modal's text comes from the catalog.

use signex_i18n::{Arg, I18n, LanguagePref, ids};

use crate::app::{Message, Signex, UiMsg};

fn caption(i18n: &I18n) -> String {
    let arg = |v: &str| Arg::from(v);
    i18n.message_plain(
        ids::PDF_PAGE_CAPTION,
        &[
            ("page", arg("1")),
            ("pages", arg("3")),
            ("width", arg("4961")),
            ("height", arg("3508")),
            ("zoom", arg("50")),
        ],
    )
}

/// The page caption keeps the English the view built before, pixel sizes
/// ungrouped.
#[test]
fn the_page_caption_keeps_its_english() {
    assert_eq!(caption(&I18n::en_us()), "Page 1 of 3 — 4961×3508 px · 50%");
}

/// A section title follows the language in force.
#[test]
fn the_pdf_text_follows_the_language() {
    let (mut app, _task) = Signex::new();
    let switch = |app: &mut Signex, tag: &str| {
        let pref = LanguagePref::Locale(tag.to_owned());
        let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
    };
    let shown = |app: &Signex| app.ui_state.i18n.message_plain(ids::PDF_SECTION_FILES, &[]);
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
    switch(&mut app, "en-XA");
    assert_ne!(shown(&app), "Choose Project Files");
    assert_ne!(caption(&app.ui_state.i18n), caption(&I18n::en_us()));
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
}
