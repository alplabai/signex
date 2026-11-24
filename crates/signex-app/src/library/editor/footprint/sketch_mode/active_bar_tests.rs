//! #2843 — the constraint strip is always whole; what the selection cannot
//! take is greyed and inert (DECISIONS 2026-11-23, mockup row 179).

use signex_library::{Footprint, FootprintFile};
use signex_types::theme::{ThemeId, theme_tokens};

use super::*;
use crate::library::editor::footprint::state::EditorMode;
use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;

fn footprint_sketch() -> FootprintDocument {
    let file = FootprintFile::from_footprint(Footprint::empty("t"));
    let mut editor = FootprintDocument::new(std::path::PathBuf::from("t.snxfpt"), file);
    apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::SetMode(EditorMode::Sketch));
    editor
}

fn board_sketch() -> FootprintDocument {
    let mut editor = footprint_sketch();
    let pcb = signex_types::project::SetupProfile::Standard
        .pcb(2)
        .new_board();
    editor.state.board_backdrop = Some(std::sync::Arc::new(
        crate::library::editor::footprint::BoardBackdrop::from_board(&pcb),
    ));
    editor
}

/// `(enabled, has a message)` of each slot of the constraint strip.
fn strip(editor: &FootprintDocument) -> Vec<(bool, bool)> {
    let tokens = theme_tokens(ThemeId::DEFAULT);
    let bar = items(
        editor,
        ThemeId::DEFAULT,
        &tokens,
        &signex_i18n::I18n::en_us(),
    );
    assert!(matches!(
        bar[CONSTRAINT_SLOTS.start - 1],
        ActiveBarItem::Separator
    ));
    assert!(matches!(
        bar[CONSTRAINT_SLOTS.end],
        ActiveBarItem::Separator
    ));
    bar[CONSTRAINT_SLOTS]
        .iter()
        .map(|item| match item {
            ActiveBarItem::Button(button) => (button.enabled, button.on_press.is_some()),
            _ => panic!("the strip holds buttons only"),
        })
        .collect()
}

#[test]
fn with_nothing_selected_all_19_buttons_show_greyed_and_inert_on_both_hosts() {
    for editor in [footprint_sketch(), board_sketch()] {
        let strip = strip(&editor);
        assert_eq!(strip.len(), 19);
        assert!(strip.iter().all(|slot| *slot == (false, false)));
    }
}

#[test]
fn a_selected_point_lights_fix_alone_and_the_strip_stays_whole() {
    let mut editor = footprint_sketch();
    apply_footprint_primitive_edit(
        &mut editor,
        FootprintEditorMsg::SketchPlacePoint {
            x_mm: 1.0,
            y_mm: 1.0,
        },
    );
    let point = editor
        .primitive()
        .sketch
        .as_ref()
        .and_then(|sketch| sketch.entities.last())
        .map(|entity| entity.id)
        .expect("the point");
    editor.state.selected_sketch = Some(point);
    editor.state.selected_sketch_secondary = None;
    editor.state.selected_sketch_extra.clear();

    let strip = strip(&editor);
    assert_eq!(strip.len(), 19);
    for (index, slot) in strip.iter().enumerate() {
        let fix = index == tag_index(SketchConstraintTag::Fixed);
        assert_eq!(*slot, (fix, fix), "slot {index}");
    }
}
