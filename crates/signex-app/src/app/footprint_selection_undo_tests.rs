//! Undo and Redo in the footprint editor give back the pad and sketch
//! selection held when the step was taken, and clear the silk selection.
//! Pinned before the selection moved out of the history step (#2326).

use std::path::PathBuf;

use signex_library::FootprintFile;
use signex_library::primitive::footprint::Footprint;
use signex_sketch::id::SketchEntityId;

use super::FootprintDocument;

/// A clean editor holding three pads, with no undo step yet.
fn editor_with_three_pads() -> FootprintDocument {
    let file = FootprintFile::from_footprint(Footprint::empty("Fp"));
    let mut editor = FootprintDocument::new(PathBuf::from("Lib.snxfpt"), file);
    for x in [0.0, 2.0, 4.0] {
        editor.state.add_pad_at(x, 0.0);
    }
    editor.set_dirty(false);
    editor
}

/// One recorded edit that renames the footprint.
fn rename(editor: &mut FootprintDocument, name: &str) {
    editor.record_edit(None, |e| {
        e.primitive_mut().name = name.into();
        e.mark_dirty();
        true
    });
}

type Picks = (
    Option<usize>,
    Option<SketchEntityId>,
    Option<SketchEntityId>,
);

fn select(editor: &mut FootprintDocument, pad: Option<usize>) -> Picks {
    let picks = (
        pad,
        Some(SketchEntityId::new()),
        Some(SketchEntityId::new()),
    );
    editor.state.selected_pad = picks.0;
    editor.state.selected_sketch = picks.1;
    editor.state.selected_sketch_secondary = picks.2;
    picks
}

fn picks(editor: &FootprintDocument) -> Picks {
    let state = &editor.state;
    (
        state.selected_pad,
        state.selected_sketch,
        state.selected_sketch_secondary,
    )
}

#[test]
fn undo_gives_back_the_selection_held_when_the_step_was_taken() {
    let mut editor = editor_with_three_pads();
    let before = select(&mut editor, Some(0));
    rename(&mut editor, "A");
    select(&mut editor, Some(2));

    assert!(editor.undo());

    assert_eq!(picks(&editor), before);
    assert_eq!(editor.primitive().name, "Fp");
}

#[test]
fn redo_gives_back_the_selection_held_when_undo_ran() {
    let mut editor = editor_with_three_pads();
    select(&mut editor, Some(0));
    rename(&mut editor, "A");
    let at_undo = select(&mut editor, Some(1));
    assert!(editor.undo());
    select(&mut editor, Some(2));

    assert!(editor.redo());

    assert_eq!(picks(&editor), at_undo);
    assert_eq!(editor.primitive().name, "A");
}

#[test]
fn each_undo_and_redo_of_a_chain_gives_back_its_own_selection() {
    let mut editor = editor_with_three_pads();
    let first = select(&mut editor, Some(0));
    rename(&mut editor, "A");
    let second = select(&mut editor, Some(1));
    rename(&mut editor, "B");
    let last = select(&mut editor, Some(2));

    assert!(editor.undo());
    assert_eq!(picks(&editor), second);
    assert!(editor.undo());
    assert_eq!(picks(&editor), first);
    assert!(editor.redo());
    assert_eq!(picks(&editor), second, "the pick held when undo ran");
    assert!(editor.redo());
    assert_eq!(picks(&editor), last);
}

#[test]
fn undo_past_the_first_step_changes_no_selection() {
    let mut editor = editor_with_three_pads();
    rename(&mut editor, "A");
    assert!(editor.undo());
    let now = select(&mut editor, Some(1));

    assert!(!editor.undo(), "nothing left to undo");

    assert_eq!(picks(&editor), now);
}

#[test]
fn an_edit_after_undo_drops_redo_and_its_selection() {
    let mut editor = editor_with_three_pads();
    rename(&mut editor, "A");
    assert!(editor.undo());
    rename(&mut editor, "B");
    let now = select(&mut editor, Some(1));

    assert!(!editor.redo(), "the new edit cut the redo branch");

    assert_eq!(picks(&editor), now);
    assert_eq!(editor.primitive().name, "B");
}

#[test]
fn a_pad_pick_is_an_index_given_back_as_held_even_past_the_last_pad() {
    let mut editor = editor_with_three_pads();
    let past_the_end = select(&mut editor, Some(9));
    editor.record_edit(None, |e| {
        e.state.pads.pop();
        e.state.selected_pad = None;
        e.mark_dirty();
        true
    });
    let gone_third = select(&mut editor, Some(2));
    assert!(editor.undo());
    assert_eq!(editor.state.pads.len(), 3);
    assert_eq!(picks(&editor), past_the_end, "not checked against the pads");

    assert!(editor.redo());

    assert_eq!(editor.state.pads.len(), 2);
    assert_eq!(picks(&editor), gone_third, "pad 2 is gone, the index stays");
}

#[test]
fn a_step_keeps_the_extra_picks_and_clears_the_silk_pick() {
    let mut editor = editor_with_three_pads();
    rename(&mut editor, "A");
    let extra_sketch = vec![SketchEntityId::new()];
    editor.state.selected_pads_extra = vec![1, 2];
    editor.state.selected_sketch_extra = extra_sketch.clone();
    editor.state.selected_silk_f = Some(0);

    assert!(editor.undo());

    assert_eq!(editor.state.selected_pads_extra, vec![1, 2]);
    assert_eq!(editor.state.selected_sketch_extra, extra_sketch);
    assert_eq!(editor.state.selected_silk_f, None);
}

#[test]
fn coalesced_typing_gives_back_the_selection_before_the_first_keystroke() {
    let mut editor = editor_with_three_pads();
    let before = select(&mut editor, Some(0));
    let type_name = |editor: &mut FootprintDocument, text: &str| {
        editor.record_edit(Some(("name", 0)), |e| {
            e.primitive_mut().name = text.into();
            e.mark_dirty();
            true
        });
    };
    type_name(&mut editor, "A");
    select(&mut editor, Some(1));
    type_name(&mut editor, "AB");
    select(&mut editor, Some(2));

    assert!(editor.undo());

    assert_eq!(picks(&editor), before);
    assert_eq!(editor.history.undo_len(), 0, "one coalesced step");
}
