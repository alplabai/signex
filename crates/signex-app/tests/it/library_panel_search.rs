//! #1373 — typing in the left-dock library search box must write the
//! panel's own buffer, with or without the Place Component modal open.

use signex_app::app::{Message, Signex};
use signex_app::library::messages::{LibraryMessage, PickerMsg};

#[test]
fn panel_search_text_lands_in_the_panel_buffer() {
    let (mut app, _t) = Signex::new();
    assert!(app.library.picker.is_none());

    drop(app.update(Message::Library(LibraryMessage::Picker(
        PickerMsg::PanelSearchChanged("res".into()),
    ))));

    assert_eq!(app.library.panel_search, "res");
}
