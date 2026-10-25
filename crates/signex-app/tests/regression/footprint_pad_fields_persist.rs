//! #960 — every pad field the Properties panel edits must survive
//! save and reopen, and the sketch copy of the pad must not be
//! overwritten with defaults by the first edit after reopen.

use signex_app::app::{FootprintEditorState, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::{Footprint, FootprintFile, Pad};
use signex_sketch::attr::PadAttr;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_sketch::sketch::SketchData;
use std::path::PathBuf;

fn open_editor(stem: &str, file: FootprintFile) -> (Signex, PathBuf) {
    let path = PathBuf::from(format!("{stem}.snxfpt"));
    let editor = FootprintEditorState::new(path.clone(), file);
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

/// A footprint with one pad whose sketch centre point carries a
/// `PadAttr`, so the Pads-mode -> sketch mirror has something to write.
fn footprint_with_sketch_pad() -> Footprint {
    let mut fp = Footprint::empty("persist");
    fp.pads.push(Pad {
        number: "1".into(),
        ..Pad::default()
    });
    let plane_id = PlaneId::new();
    let mut sketch = SketchData::default();
    sketch.planes.push(Plane {
        id: plane_id,
        kind: PlaneKind::BoardTop,
    });
    let mut centre = Entity::new(
        SketchEntityId::new(),
        plane_id,
        EntityKind::Point { x: 0.0, y: 0.0 },
    );
    centre.pad = Some(PadAttr {
        number: "1".into(),
        ..PadAttr::default()
    });
    sketch.entities.push(centre);
    fp.sketch = Some(sketch);
    fp
}

#[test]
fn pad_panel_edits_survive_save_reopen_and_next_edit() {
    let (mut app, path) = open_editor(
        "persist",
        FootprintFile::from_footprint(footprint_with_sketch_pad()),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadPasteMarginTop {
            idx: 0,
            value: "-0.04".into(),
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadMaskTentedTop {
            idx: 0,
            value: true,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadTestpointTopAssembly {
            idx: 0,
            value: true,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadLocked {
            idx: 0,
            value: true,
        },
    );

    // Save, then reopen from the bytes on disk.
    let saved = app
        .document_state
        .footprint_editors
        .get(&path)
        .unwrap()
        .file
        .to_toml_string()
        .expect("serialise");
    let reopened = FootprintFile::from_toml_str(&saved).expect("reparse");
    let (mut app, path) = open_editor("persist-reopened", reopened);

    // The next pad-mode edit runs sync_pads_to_primitive, which mirrors
    // the editor pad onto the sketch's PadAttr.
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadNet {
            idx: 0,
            value: "GND".into(),
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.primitive().pads[0];
    assert_eq!(pad.paste_margin_top, Some(-0.04));
    assert!(pad.mask_tented_top);
    assert!(pad.testpoint.top_assembly);
    assert!(pad.locked);
    assert_eq!(pad.net, "GND");

    let attr = editor
        .primitive()
        .sketch
        .as_ref()
        .unwrap()
        .entities
        .iter()
        .find_map(|e| e.pad.as_ref())
        .unwrap();
    assert!(
        attr.testpoint.top_assembly && attr.locked,
        "sketch PadAttr must keep the reopened values, not Pad::default()"
    );
}

/// The solve rebakes `footprint.pads` from `PadAttr`. A pad-stack edit
/// made in Pads mode must therefore reach `PadAttr.stack`, or the next
/// sketch edit puts the old value back.
#[test]
fn pad_stack_edits_survive_a_sketch_solve() {
    use signex_app::library::editor::footprint::pad_to_sketch::mirror_add_pad_to_sketch;
    use signex_app::library::editor::footprint::state::EditorPad;
    use signex_app::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

    let mut pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    let mut fp = Footprint::empty("stack");
    mirror_add_pad_to_sketch(&mut pad, &mut fp);
    let (mut app, path) = open_editor("stack", FootprintFile::from_footprint(fp));
    app.document_state
        .footprint_editors
        .get_mut(&path)
        .unwrap()
        .state
        .pads = vec![pad];

    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadMaskTentedTop {
            idx: 0,
            value: true,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadPasteEnabledBottom {
            idx: 0,
            value: false,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadThermalRelief {
            idx: 0,
            value: false,
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadPasteMarginTop {
            idx: 0,
            value: "-0.04".into(),
        },
    );
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadMaskMarginBottom {
            idx: 0,
            value: "0.06".into(),
        },
    );

    // Any sketch edit solves and rebakes the pads from the sketch.
    drop(
        app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchEditParameter {
                name: "unrelated".into(),
                expr: "1mm".into(),
            }),
        })),
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let pad = &editor.primitive().pads[0];
    assert!(pad.mask_tented_top, "tented survives the solve");
    assert!(!pad.paste_enabled_bottom, "paste-disable survives");
    assert!(!pad.thermal_relief, "direct-connect survives");
    assert_eq!(pad.paste_margin_top, Some(-0.04));
    assert_eq!(pad.mask_margin_bottom, Some(0.06));
}

/// An untouched editor pad (paste on, thermal relief on) must not make
/// the file grow extended columns.
#[test]
fn editor_default_pad_saves_only_the_base_columns() {
    use signex_app::library::editor::footprint::state::{
        EditorPad, FootprintEditorState as CanvasState,
    };
    let pad = EditorPad::new_default("1".into(), (0.0, 0.0));
    let mut fp = Footprint::empty("plain");
    CanvasState::sync_pads_to_primitive(
        &{
            let mut s = CanvasState::empty();
            s.pads = vec![pad];
            s
        },
        &mut fp,
    );
    let text = FootprintFile::from_footprint(fp).to_toml_string().unwrap();
    let header = text
        .lines()
        .skip_while(|l| !l.starts_with("pads_tsv"))
        .nth(1)
        .unwrap();
    assert_eq!(header.split('\t').count(), 13, "header was {header:?}");
}

fn sketch_attr(fp: &mut Footprint) -> &mut PadAttr {
    fp.sketch
        .as_mut()
        .unwrap()
        .entities
        .iter_mut()
        .find_map(|e| e.pad.as_mut())
        .unwrap()
}

/// What a Sketch-mode session leaves behind: stack and attribute values
/// that exist ONLY on the sketch `PadAttr` (the `Pad` list is default).
fn footprint_with_sketch_only_pad_state() -> Footprint {
    use signex_sketch::attr::TestpointFlags;
    let mut fp = footprint_with_sketch_pad();
    let attr = sketch_attr(&mut fp);
    attr.stack.mask_top_tented = true;
    attr.stack.paste_top_enabled = false;
    attr.stack.thermal_relief = false;
    attr.stack.mask_bottom_expr = Some("0.06mm".into());
    attr.stack.paste_bottom_expr = Some("-0.02mm".into());
    attr.stack.corner_radius_pct = Some(20.0);
    attr.net = "VCC".into();
    attr.testpoint = TestpointFlags {
        top_fab: true,
        ..TestpointFlags::default()
    };
    fp
}

fn assert_sketch_only_state(attr: &PadAttr, when: &str) {
    assert!(attr.stack.mask_top_tented, "{when}: tented");
    assert!(!attr.stack.paste_top_enabled, "{when}: paste disabled");
    assert!(!attr.stack.thermal_relief, "{when}: direct connect");
    assert_eq!(attr.stack.corner_radius_pct, Some(20.0), "{when}: radius");
    assert_eq!(attr.net, "VCC", "{when}: net");
    assert!(attr.testpoint.top_fab, "{when}: testpoint");
    let mm = |e: &Option<String>| {
        e.as_deref()
            .and_then(|s| s.strip_suffix("mm"))
            .and_then(|n| n.parse::<f64>().ok())
    };
    assert_eq!(mm(&attr.stack.mask_bottom_expr), Some(0.06), "{when}: mask");
    assert_eq!(
        mm(&attr.stack.paste_bottom_expr),
        Some(-0.02),
        "{when}: paste"
    );
}

/// A file saved before #960 has no extended pad columns, so the `Pad`
/// list holds defaults while the sketch `PadAttr` holds the real values.
/// The first Pads-mode edit must not overwrite the sketch with defaults.
#[test]
fn pre_960_file_keeps_sketch_only_pad_state_through_a_pads_edit() {
    let mut fp = footprint_with_sketch_only_pad_state();
    // Pre-#960 shape: the Pad list is default, so only 13 columns are written.
    fp.pads = vec![Pad {
        number: "1".into(),
        ..Pad::default()
    }];
    let text = FootprintFile::from_footprint(fp).to_toml_string().unwrap();
    let header = text
        .lines()
        .skip_while(|l| !l.starts_with("pads_tsv"))
        .nth(1)
        .unwrap();
    assert_eq!(
        header.split('\t').count(),
        13,
        "fixture must be a 13-column file"
    );
    let (mut app, path) = open_editor(
        "pre960",
        FootprintFile::from_toml_str(&text).expect("reparse"),
    );

    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadLocked {
            idx: 0,
            value: true,
        },
    );

    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let mut fp = editor.primitive().clone();
    assert_sketch_only_state(sketch_attr(&mut fp), "after the Pads-mode edit");

    // Save, reopen, edit again: still intact, and the Pad list now carries it.
    let saved = editor.file.to_toml_string().unwrap();
    let (mut app, path) = open_editor(
        "pre960-reopened",
        FootprintFile::from_toml_str(&saved).unwrap(),
    );
    panel(
        &mut app,
        PanelMsg::FpEditorToggleSelectedPadLocked {
            idx: 0,
            value: false,
        },
    );
    let editor = app.document_state.footprint_editors.get(&path).unwrap();
    let mut fp = editor.primitive().clone();
    assert_sketch_only_state(sketch_attr(&mut fp), "after save, reopen, edit");
    let pad = &editor.primitive().pads[0];
    assert!(pad.mask_tented_top && !pad.paste_enabled_top && !pad.thermal_relief);
    assert_eq!(pad.net, "VCC");
}

/// `paste_*_pct` and `paste_*_expr` are mutually exclusive: an editor
/// margin that writes the expression must clear the percentage.
#[test]
fn pads_mode_paste_margin_clears_the_exclusive_percentage() {
    let mut fp = footprint_with_sketch_pad();
    sketch_attr(&mut fp).stack.paste_top_pct = Some(10.0);
    let (mut app, path) = open_editor("pct", FootprintFile::from_footprint(fp));
    panel(
        &mut app,
        PanelMsg::FpEditorSetSelectedPadPasteMarginTop {
            idx: 0,
            value: "-0.04".into(),
        },
    );
    let mut fp = app
        .document_state
        .footprint_editors
        .get(&path)
        .unwrap()
        .primitive()
        .clone();
    let stack = &sketch_attr(&mut fp).stack;
    assert_eq!(stack.paste_top_expr.as_deref(), Some("-0.04mm"));
    assert_eq!(stack.paste_top_pct, None);
}
