//! #2058: the Export PDF modal's text comes from the catalog.

use signex_i18n::{Arg, I18n, LanguagePref, ids};

use crate::app::{Message, Signex, UiMsg};
use crate::layout_probe::lay_out_texts;

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
    // The text the section title widget lays out.
    let shown = |app: &Signex| {
        let window = iced::Size::new(400.0, 100.0);
        lay_out_texts(app.pdf_section_title(ids::PDF_SECTION_FILES), window)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect::<Vec<_>>()
            .join("|")
    };
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
    switch(&mut app, "en-XA");
    assert_ne!(shown(&app), "Choose Project Files");
    assert_eq!(
        shown(&app),
        app.ui_state.i18n.message_plain(ids::PDF_SECTION_FILES, &[])
    );
    assert_ne!(caption(&app.ui_state.i18n), caption(&I18n::en_us()));
    switch(&mut app, "en-US");
    assert_eq!(shown(&app), "Choose Project Files");
}
