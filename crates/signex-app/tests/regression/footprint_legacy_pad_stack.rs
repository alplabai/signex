//! #1073 — a footprint authored before #1052 has every sketch pad saved
//! with paste disabled on both sides and direct connect. Opening it offers
//! a one-time restore (undoable, never saved for the user); a standing check
//! names SMD pads with no paste opening.

use signex_app::app::{FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::pad_stack_check;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Pad};
use signex_sketch::attr::{PadAttr, PadStackOverrides};
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use std::path::PathBuf;

/// The stack the derived `Default` wrote before #1052.
fn pre_1052_stack() -> PadStackOverrides {
    PadStackOverrides {
        paste_top_enabled: false,
        paste_bottom_enabled: false,
        thermal_relief: false,
        ..PadStackOverrides::default()
    }
}

fn footprint_with_pads(stacks: &[PadStackOverrides]) -> Footprint {
    let mut fp = Footprint::empty("legacy");
    let plane_id = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane_id,
        kind: PlaneKind::BoardTop,
    });
    for (i, stack) in stacks.iter().enumerate() {
        let number = (i + 1).to_string();
        // A default `Pad` carries none of the extended columns, so the
        // saved pad table has the 13 columns a pre-#1052 build wrote.
        fp.pads.push(Pad {
            number: number.clone(),
            position: [i as f64 * 2.0, 0.0],
            ..Pad::default()
        });
        let mut centre = Entity::new(
            SketchEntityId::new(),
            plane_id,
            EntityKind::Point {
                x: i as f64 * 2.0,
                y: 0.0,
            },
        );
        centre.pad = Some(PadAttr {
            number,
            stack: stack.clone(),
            ..PadAttr::default()
        });
        sketch.entities.push(centre);
    }
    fp.sketch = Some(sketch);
    fp
}

/// Round-trips through the TOML a saved file holds, like a real open.
fn reopen(fp: Footprint) -> FootprintFile {
    let saved = FootprintFile::from_footprint(fp)
        .to_toml_string()
        .expect("serialise");
    FootprintFile::from_toml_str(&saved).expect("reparse")
}

fn open_editor(file: FootprintFile) -> (Signex, PathBuf) {
    let path = PathBuf::from("legacy.snxfpt");
    let editor = FootprintEditorState::new(path.clone(), file);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "legacy".into(),
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

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintEditorState {
    app.document_state.footprint_editors.get(path).unwrap()
}

fn stacks(app: &Signex, path: &PathBuf) -> Vec<PadStackOverrides> {
    editor(app, path)
        .primitive()
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .filter_map(|e| e.pad.as_ref().map(|a| a.stack.clone()))
        .collect()
}

#[test]
fn a_pre_1052_footprint_is_offered_the_restore_and_nothing_is_changed_on_open() {
    let (app, path) = open_editor(reopen(footprint_with_pads(&[
        pre_1052_stack(),
        pre_1052_stack(),
    ])));
    let ed = editor(&app, &path);
    assert!(ed.state.legacy_pad_stack_offered, "offer is raised on open");
    assert!(!ed.dirty, "opening alone must not dirty the document");
    assert!(stacks(&app, &path).iter().all(|s| !s.paste_top_enabled));
    assert_eq!(
        pad_stack_check::smd_pads_without_paste(&ed.state.pads),
        ["1", "2"],
        "the standing check names both pads"
    );
}

#[test]
fn restore_enables_paste_and_thermal_in_one_undo_step() {
    let (mut app, path) = open_editor(reopen(footprint_with_pads(&[
        pre_1052_stack(),
        pre_1052_stack(),
    ])));
    panel(&mut app, PanelMsg::FpEditorRestorePadStackDefaults);

    let ed = editor(&app, &path);
    assert!(ed.dirty, "the restore is an unsaved edit");
    for s in stacks(&app, &path) {
        assert!(s.paste_top_enabled && s.paste_bottom_enabled && s.thermal_relief);
    }
    for pad in &ed.primitive().pads {
        assert!(pad.paste_enabled_top && pad.paste_enabled_bottom && pad.thermal_relief);
    }
    assert!(pad_stack_check::smd_pads_without_paste(&ed.state.pads).is_empty());
    assert_eq!(ed.history.len(), 1, "exactly one undo step");

    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .undo();
    let ed = editor(&app, &path);
    assert!(stacks(&app, &path).iter().all(|s| *s == pre_1052_stack()));
    assert_eq!(
        pad_stack_check::smd_pads_without_paste(&ed.state.pads).len(),
        2,
        "undo brings the warning back"
    );
}

#[test]
fn a_footprint_with_one_deliberate_pad_is_not_offered_the_restore() {
    let (app, path) = open_editor(reopen(footprint_with_pads(&[
        pre_1052_stack(),
        PadStackOverrides::default(),
    ])));
    assert!(!editor(&app, &path).state.legacy_pad_stack_offered);
}

#[test]
fn dismissing_the_offer_changes_nothing_in_the_document() {
    let (mut app, path) = open_editor(reopen(footprint_with_pads(&[pre_1052_stack()])));
    panel(&mut app, PanelMsg::FpEditorDismissPadStackOffer);
    let ed = editor(&app, &path);
    assert!(!ed.state.legacy_pad_stack_offered);
    assert!(!ed.dirty);
    assert!(ed.history.is_empty());
    assert!(stacks(&app, &path).iter().all(|s| *s == pre_1052_stack()));
    // The standing warning stays: the pad still has no paste opening.
    assert_eq!(
        pad_stack_check::smd_pads_without_paste(&ed.state.pads),
        ["1"]
    );
}
