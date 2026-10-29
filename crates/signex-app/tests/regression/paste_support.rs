//! Driving a paste in a headless test: the request goes through
//! `EditMsg::Paste`, and the system clipboard's answer — which only a running
//! window can give — is delivered by hand.

use signex_app::app::{EditMsg, Message, PasteIntent, Signex};

/// `text` as the system clipboard's answer to a paste request.
pub(crate) fn paste_with_clipboard(app: &mut Signex, text: Option<&str>) {
    let _read_request = app.update(Message::Edit(EditMsg::Paste));
    answer(app, PasteIntent::Paste, text);
}

/// A paste with nothing usable on the system clipboard: the copy made in the
/// window is pasted.
pub(crate) fn paste(app: &mut Signex) {
    paste_with_clipboard(app, None);
}

/// Smart Paste with nothing usable on the system clipboard.
pub(crate) fn smart_paste(app: &mut Signex) {
    let _read_request = app.update(Message::Edit(EditMsg::SmartPaste));
    answer(app, PasteIntent::SmartPaste, None);
}

pub(crate) fn answer(app: &mut Signex, intent: PasteIntent, text: Option<&str>) {
    let _ = app.update(Message::Edit(EditMsg::PasteTextRead {
        intent,
        text: text.map(str::to_owned),
    }));
}
