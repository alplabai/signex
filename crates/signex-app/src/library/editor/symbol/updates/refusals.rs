//! Edits the symbol editor refuses (#3129): any change to a file that opened
//! repaired, until a save writes the repair, and pin text a save could not
//! store. The panel edits call the same two refusals as the canvas ones.

use signex_i18n::{I18n, Localized, ids, t};
use signex_library::UnstorablePinText;

use super::SymEditor;
use crate::library::messages::SymbolEditorMsg;

/// Run `apply` on a read-only draft and take back whatever it changed in the
/// document; camera, tool and selection messages still act.
pub(super) fn apply_read_only(
    editor: &mut SymEditor,
    msg: SymbolEditorMsg,
    apply: fn(&mut SymEditor, SymbolEditorMsg),
) {
    let before = editor.draft.clone();
    apply(editor, msg);
    if editor.draft.file != before.file {
        editor.draft = before;
        // The edit may have selected what it added.
        editor.gui.selected = None;
        editor.gui.canvas_cache.clear();
        refuse_read_only(editor);
    }
}

/// Say in the status line why the edit was not applied.
pub(crate) fn refuse_read_only(editor: &mut SymEditor) {
    let line = t!(I18n::en_us_ref(), ids::EDITORS_SYMBOL_READ_ONLY_REPAIRED);
    editor.gui.status_message = Some(line.into_string());
}

/// The pin text the change from `before` to `after` brought in that a save
/// would refuse; text `before` already held is not counted again.
pub(crate) fn new_unstorable_pin_text(
    before: &signex_library::Symbol,
    after: &signex_library::Symbol,
) -> Option<UnstorablePinText> {
    let refusal = after.first_unstorable_pin_text()?;
    before
        .first_unstorable_pin_text()
        .is_none()
        .then_some(refusal)
}

/// Keep the refusal for the Properties panel and the status line.
pub(crate) fn refuse_pin_text(editor: &mut SymEditor, refusal: UnstorablePinText) {
    let pin = editor
        .primitive()
        .pins
        .get(refusal.pin_index)
        .map(|p| p.number.clone())
        .unwrap_or_default();
    let line = pin_text_refused(&refusal, &pin, I18n::en_us_ref());
    editor.gui.status_message = Some(line.into_string());
    editor.gui.pin_text_refusal = Some(refusal);
}

/// "The name of pin 7 cannot hold the control character U+000D; …".
pub(crate) fn pin_text_refused(refusal: &UnstorablePinText, pin: &str, i18n: &I18n) -> Localized {
    t!(
        i18n,
        ids::EDITORS_SYMBOL_PIN_TEXT_REFUSED,
        "column" => refusal.column,
        "pin" => pin,
        "character" => signex_library::toml_literal::code_point(refusal.character)
    )
}
