//! The symbol editor's segment of the global status bar: the active part,
//! its pin count and what is selected, or the editor's last error.
//! The rest of the bar reads the symbol tab too: X / Y and zoom from the
//! editor, the tool from `editor.tool`, and the grid and unit from the
//! library's Document Options, which its Grid and unit buttons toggle.

use signex_i18n::{I18n, ids, t};

use crate::app::SymbolDocument;
use crate::library::editor::symbol::state::{self as sym_state, SymbolSelection};
use crate::status_bar::Activity;

/// The status bar segment for `editor`. An error the editor reported wins
/// and is drawn as an error; otherwise "Part 1 / 2 · 8 pins" and the
/// selection, or the key hints.
pub(crate) fn segment(editor: &SymbolDocument, i18n: &I18n) -> Activity {
    if let Some(message) = &editor.status_message {
        return Activity {
            text: message.clone(),
            is_error: true,
        };
    }
    Activity::note(summary(editor, i18n))
}

/// "Part 1 / 2 · 8 pins" and the selection, or the key hints.
fn summary(editor: &SymbolDocument, i18n: &I18n) -> String {
    let symbol = editor.primitive();
    let view = editor.view();
    let pins = symbol
        .pins
        .iter()
        .filter(|pin| sym_state::pin_on_part(pin, view))
        .count();
    let part = t!(
        i18n,
        ids::EDITORS_SYMBOL_STATUS_PART_PINS,
        "part" => u32::from(editor.active_part),
        "parts" => u32::from(sym_state::max_part_number(symbol)),
        "pins" => pins
    );
    let tail = match &editor.selected {
        Some(selection) => format!(
            "{} · {}",
            selection_text(editor, selection, i18n),
            t!(i18n, ids::EDITORS_SYMBOL_STATUS_HINT_SELECTED)
        ),
        None => t!(i18n, ids::EDITORS_SYMBOL_STATUS_HINT_IDLE).into_string(),
    };
    format!("{part} · {tail}")
}

/// "Pin 8 VOUT selected" for one pin, "3 selected" for anything else.
fn selection_text(editor: &SymbolDocument, selection: &SymbolSelection, i18n: &I18n) -> String {
    let symbol = editor.primitive();
    let count = match selection {
        SymbolSelection::Pin(idx) => {
            if let Some(pin) = symbol.pins.get(*idx) {
                return t!(
                    i18n,
                    ids::EDITORS_SYMBOL_STATUS_PIN_SELECTED,
                    "number" => pin.number.as_str(),
                    "name" => pin.name.as_str()
                )
                .into_string();
            }
            1
        }
        SymbolSelection::Field(_) | SymbolSelection::Graphic(_) => 1,
        SymbolSelection::All => symbol.pins.len() + symbol.graphics.len(),
        SymbolSelection::Multiple {
            pin_indices,
            graphic_indices,
        } => pin_indices.len() + graphic_indices.len(),
    };
    t!(i18n, ids::EDITORS_SYMBOL_STATUS_ITEMS_SELECTED, "count" => count).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn editor_with_pins(pins: &[(&str, &str, u8)]) -> SymbolDocument {
        let mut symbol = signex_library::Symbol::empty("s");
        for (number, name, part) in pins {
            let mut pin = signex_library::SymbolPin::new(*number, *name);
            pin.part_number = *part;
            symbol.pins.push(pin);
        }
        let file = signex_library::SymbolFile::from_symbol(symbol);
        SymbolDocument::new(PathBuf::from("s.snxsym"), file)
    }

    fn i18n() -> std::sync::Arc<I18n> {
        crate::app::Signex::new().0.ui_state.i18n.clone()
    }

    #[test]
    fn the_segment_counts_the_active_parts_pins_and_hints_at_the_keys() {
        let editor = editor_with_pins(&[("1", "VIN", 1), ("2", "GND", 2), ("3", "EN", 1)]);
        let i18n = i18n();
        let segment = segment(&editor, &i18n);
        assert!(!segment.is_error);
        let segment = segment.text;
        let part = t!(
            i18n,
            ids::EDITORS_SYMBOL_STATUS_PART_PINS,
            "part" => 1u32,
            "parts" => 2u32,
            "pins" => 2usize
        );
        let hint = t!(i18n, ids::EDITORS_SYMBOL_STATUS_HINT_IDLE);
        assert_eq!(segment, format!("{part} · {hint}"));
    }

    #[test]
    fn a_selected_pin_is_named_by_number_and_name() {
        let mut editor = editor_with_pins(&[("1", "VIN", 1), ("8", "VOUT", 1)]);
        editor.selected = Some(SymbolSelection::Pin(1));
        let i18n = i18n();
        let shown = segment(&editor, &i18n).text;
        assert!(shown.contains("8") && shown.contains("VOUT"), "{shown}");
    }

    #[test]
    fn an_editor_error_replaces_the_segment() {
        let mut editor = editor_with_pins(&[("1", "VIN", 1)]);
        editor.status_message = Some("Arc sweep rejected".into());
        let shown = segment(&editor, &i18n());
        assert_eq!(shown.text, "Arc sweep rejected");
        assert!(shown.is_error, "the error is drawn as an error");
    }
}
