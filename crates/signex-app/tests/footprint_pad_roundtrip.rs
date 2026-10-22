//! A footprint edit must not erase data the loaded file carried, and a
//! Properties-panel edit must be undoable like any canvas edit (#667).
//!
//! `EditorPad` is a canvas-side mirror of `signex_library::Pad` that is
//! written back after every mutation. Any `Pad` field the mirror does
//! not carry is reset to its default on the first edit, so a slotted
//! drill or a legacy mask / paste margin silently vanished from the
//! saved file.

use signex_app::app::{EditMsg, FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_library::{Drill, Footprint, FootprintFile, Pad, PadKind};
use std::path::PathBuf;

/// The pad's persisted fields the editor has no UI for.
fn slotted_pad_with_margins() -> Pad {
    Pad {
        number: "1".into(),
        kind: PadKind::Tht,
        size: [1.6, 2.6],
        drill: Some(Drill {
            diameter: 0.9,
            slot_length: Some(1.8),
        }),
        solder_mask_margin: Some(0.07),
        paste_margin: Some(-0.03),
        ..Pad::default()
    }
}

/// App with a footprint editor open on `fp`, active tab pointed at it.
fn fixture(stem: &str, fp: Footprint) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintEditorState::new(path.clone(), FootprintFile::from_footprint(fp));
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: stem.into(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.active_tab = 0;
    (app, path)
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    drop(app.update(Message::Dock(DockMessage::Panel(msg))));
}

#[test]
fn editing_a_pad_keeps_slot_length_and_legacy_margins() {
    let mut fp = Footprint::empty("slotted");
    fp.pads.push(slotted_pad_with_margins());
    let (mut app, path) = fixture("slotted", fp);

    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadNet {
            idx: 0,
            value: "GND".into(),
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.primitive().pads[0];
    assert_eq!(pad.net, "GND", "pre-condition: the edit reached the pad");
    assert_eq!(pad.drill.as_ref().and_then(|d| d.slot_length), Some(1.8));
    assert_eq!(pad.solder_mask_margin, Some(0.07));
    assert_eq!(pad.paste_margin, Some(-0.03));
}

#[test]
fn properties_panel_description_edit_is_undoable() {
    let (mut app, path) = fixture("describe", Footprint::empty("describe"));

    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    );
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(editor.primitive().description, "edited");

    drop(app.update(Message::Edit(EditMsg::Undo)));

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    assert_eq!(
        editor.primitive().description,
        "",
        "Undo must revert the Properties-panel edit"
    );
}

fn editor_of<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintEditorState {
    app.document_state.footprint_editors.get(path).unwrap()
}

#[test]
fn properties_panel_pad_edit_is_undoable() {
    let mut fp = Footprint::empty("pad_undo");
    fp.pads.push(slotted_pad_with_margins());
    let (mut app, path) = fixture("pad_undo", fp);

    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadNet {
            idx: 0,
            value: "GND".into(),
        },
    );
    assert_eq!(editor_of(&app, &path).primitive().pads[0].net, "GND");

    drop(app.update(Message::Edit(EditMsg::Undo)));

    let pad = &editor_of(&app, &path).primitive().pads[0];
    assert_eq!(pad.net, "", "Undo must revert the pad Net edit");
    assert_eq!(pad.drill.as_ref().and_then(|d| d.slot_length), Some(1.8));
}

#[test]
fn properties_panel_silk_edit_is_undoable() {
    let mut fp = Footprint::empty("silk_undo");
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        },
        stroke_width: 0.1,
        filled: false,
    });
    let (mut app, path) = fixture("silk_undo", fp);
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .selected_silk_f = Some(0);

    panel(&mut app, PanelMsg::FpEditorSetSilkStrokeWidth("0.4".into()));
    assert_eq!(
        editor_of(&app, &path).primitive().silk_f[0].stroke_width,
        0.4
    );

    drop(app.update(Message::Edit(EditMsg::Undo)));

    assert_eq!(
        editor_of(&app, &path).primitive().silk_f[0].stroke_width,
        0.1,
        "Undo must revert the silk stroke-width edit"
    );
}

#[test]
fn typing_into_one_field_is_one_undo_step() {
    let (mut app, path) = fixture("typing", Footprint::empty("typing"));

    for typed in ["a", "ab", "abc"] {
        panel(
            &mut app,
            PanelMsg::FpEditorSetFootprintDescription(typed.into()),
        );
    }
    assert_eq!(editor_of(&app, &path).history.len(), 1);

    drop(app.update(Message::Edit(EditMsg::Undo)));

    assert_eq!(
        editor_of(&app, &path).primitive().description,
        "",
        "one Undo must revert the whole typed value"
    );
}

#[test]
fn same_field_on_two_footprints_is_two_undo_steps() {
    let (mut app, path) = fixture("multi", Footprint::empty("a"));
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .file
        .footprints
        .push(Footprint::empty("b"));

    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("for a".into()),
    );
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .active_idx = 1;
    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("for b".into()),
    );

    assert_eq!(
        editor_of(&app, &path).history.len(),
        2,
        "editing B's description must not merge into A's undo step"
    );
}

#[test]
fn rejected_panel_input_keeps_the_redo_stack() {
    let (mut app, path) = fixture("redo_kept", Footprint::empty("redo_kept"));
    panel(
        &mut app,
        PanelMsg::FpEditorSetFootprintDescription("edited".into()),
    );
    drop(app.update(Message::Edit(EditMsg::Undo)));
    assert_eq!(editor_of(&app, &path).redo.len(), 1, "pre-condition");
    let history_before = editor_of(&app, &path).history.len();

    // Names a sketch entity that does not exist, and an unparsable
    // priority: neither changes anything, so neither may cost the redo.
    panel(
        &mut app,
        PanelMsg::FpEditorSetPourPriority {
            id: signex_sketch::id::SketchEntityId::new(),
            value: "7".into(),
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadNet {
            idx: 99,
            value: "GND".into(),
        },
    );

    let editor = editor_of(&app, &path);
    assert_eq!(editor.redo.len(), 1, "a no-op edit must not clear Redo");
    assert_eq!(editor.history.len(), history_before, "no phantom undo step");
}
