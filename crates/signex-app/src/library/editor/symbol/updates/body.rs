//! Symbol editor — De Morgan body selection and assignment (#1233).

use super::{SymEditor, close_pickers, commit_or_discard_polygon, mark_dirty, push_undo};
use crate::library::editor::symbol::state::{assign_body, body_assign_targets};
use crate::library::messages::SymbolEditorMsg;

pub(super) fn apply_symbol_body(editor: &mut SymEditor, msg: SymbolEditorMsg) {
    match msg {
        SymbolEditorMsg::BodyStyleSelected(body) => {
            if editor.active_body == body {
                return;
            }
            // Same synchronous flush as a part switch: a staged polygon
            // belongs to the body it was drawn on.
            commit_or_discard_polygon(editor);
            editor.active_body = body;
            // Drop any selection so it can't dangle on an item the other
            // body hides.
            editor.selected = None;
            close_pickers(editor);
            editor.canvas_cache.clear();
        }
        SymbolEditorMsg::SelectionBodyAssigned(tag) => {
            let view = editor.view();
            let targets = body_assign_targets(editor.primitive(), &editor.selected, view);
            // Dry-run on a copy so a no-op pushes no undo entry.
            if !assign_body(&mut editor.primitive().clone(), &targets, tag) {
                return;
            }
            push_undo(editor);
            assign_body(editor.primitive_mut(), &targets, tag);
            // Items moved to the other body leave the view, so a selection
            // naming them would dangle.
            if tag != view.body_tag() && tag != signex_library::BODY_COMMON {
                editor.selected = None;
            }
            close_pickers(editor);
            mark_dirty(editor);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::editor::symbol::state::SymbolSelection;
    use signex_library::{BODY_ALTERNATE, BODY_COMMON, BODY_NORMAL, Symbol, SymbolFile, SymbolPin};
    use signex_types::schematic::BodyStyle;
    use std::path::PathBuf;

    fn editor_with_pin() -> SymEditor {
        let mut s = Symbol::empty("T");
        s.pins.push(SymbolPin::new("1", "A"));
        SymEditor::new(PathBuf::from("t.snxsym"), SymbolFile::from_symbol(s))
    }

    #[test]
    fn switching_body_hides_normal_items_and_stamps_new_ones_alternate() {
        let mut editor = editor_with_pin();
        apply_symbol_body(
            &mut editor,
            SymbolEditorMsg::BodyStyleSelected(BodyStyle::Alternate),
        );
        assert!(!crate::library::editor::symbol::state::pin_on_part(
            &editor.primitive().pins[0],
            editor.view()
        ));
        let view = editor.view();
        let idx =
            crate::library::editor::symbol::state::add_pin(editor.primitive_mut(), 0.0, 0.0, view);
        assert_eq!(editor.primitive().pins[idx].body_style, BODY_ALTERNATE);
    }

    #[test]
    fn marking_the_selection_common_shows_it_on_both_bodies_and_undoes() {
        let mut editor = editor_with_pin();
        editor.selected = Some(SymbolSelection::Pin(0));
        apply_symbol_body(
            &mut editor,
            SymbolEditorMsg::SelectionBodyAssigned(BODY_COMMON),
        );
        assert_eq!(editor.primitive().pins[0].body_style, BODY_COMMON);
        assert!(editor.selected.is_some(), "common items stay in view");
        apply_symbol_body(
            &mut editor,
            SymbolEditorMsg::BodyStyleSelected(BodyStyle::Alternate),
        );
        assert!(crate::library::editor::symbol::state::pin_on_part(
            &editor.primitive().pins[0],
            editor.view()
        ));
        editor.selected = Some(SymbolSelection::Pin(0));
        crate::library::editor::symbol::updates::apply_symbol_primitive_edit(
            &mut editor,
            SymbolEditorMsg::Undo,
        );
        assert_eq!(editor.primitive().pins[0].body_style, BODY_NORMAL);
    }

    #[test]
    fn assigning_what_is_already_assigned_records_nothing() {
        let mut editor = editor_with_pin();
        editor.selected = Some(SymbolSelection::Pin(0));
        apply_symbol_body(
            &mut editor,
            SymbolEditorMsg::SelectionBodyAssigned(BODY_NORMAL),
        );
        assert!(!editor.is_dirty());
    }

    #[test]
    fn moving_the_selection_to_the_other_body_clears_the_selection() {
        let mut editor = editor_with_pin();
        editor.selected = Some(SymbolSelection::Pin(0));
        apply_symbol_body(
            &mut editor,
            SymbolEditorMsg::SelectionBodyAssigned(BODY_ALTERNATE),
        );
        assert_eq!(editor.primitive().pins[0].body_style, BODY_ALTERNATE);
        assert!(editor.selected.is_none());
    }

    #[test]
    fn hit_test_only_finds_pins_of_the_shown_body() {
        use crate::library::editor::symbol::state::{
            PartView, SymbolSelectionFilter, hit_test_pin,
        };
        let mut editor = editor_with_pin();
        editor.primitive_mut().pins[0].body_style = BODY_ALTERNATE;
        let filter = SymbolSelectionFilter::default();
        let at = editor.primitive().pins[0].position;
        let hit =
            |view: PartView| hit_test_pin(editor.primitive(), at[0], at[1], 1.0, view, &filter);
        assert_eq!(hit(PartView::new(1, BodyStyle::Alternate)), Some(0));
        assert_eq!(hit(PartView::normal(1)), None);
    }
}
