//! #678 — `f64::from_str` accepts `NaN` and `inf`, so a footprint-editor
//! numeric field used to store them straight into pad / silk geometry.
//! Every field must refuse a non-finite value, keep what was stored, and
//! tell the user through the Messages panel.

#![expect(
    clippy::let_underscore_must_use,
    reason = "integration tests discard Results with `let _ = ...` routinely; this is test scaffolding, not a production `iced::Task` being dropped, and `[lints]` in Cargo.toml is package-scoped so each integration-test crate root needs its own"
)]

use signex_app::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::library::editor::footprint::state::EditorPad;
use signex_app::panels::PanelMsg;
use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_library::{Footprint, FootprintFile};
use std::path::PathBuf;

const BAD_VALUES: [&str; 4] = ["nan", "NaN", "inf", "-inf"];

fn fixture() -> (Signex, PathBuf) {
    let mut fp = Footprint::empty("non_finite");
    fp.silk_f.push(FpGraphic {
        kind: FpGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        },
        stroke_width: 0.1,
        filled: false,
    });
    let path = PathBuf::from("non_finite.snxfpt");
    let mut editor = FootprintDocument::new(path.clone(), FootprintFile::from_footprint(fp));
    editor
        .state
        .pads
        .push(EditorPad::new_default("1".into(), (0.0, 0.0)));
    editor.state.selected_silk_f = Some(0);
    let (mut app, _initial_task) = Signex::new();
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "non_finite".into(),
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
    let _ = app.update(Message::Dock(DockMessage::Panel(msg)));
}

fn editor<'a>(app: &'a Signex, path: &PathBuf) -> &'a FootprintDocument {
    app.document_state.footprint_editors.get(path).unwrap()
}

#[test]
fn next_pad_defaults_refuse_non_finite_values() {
    for bad in BAD_VALUES {
        let (mut app, path) = fixture();
        let before = editor(&app, &path).state.next_pad_defaults.clone();

        panel(&mut app, PanelMsg::FpEditorSetNextPadSizeX(bad.into()));
        panel(&mut app, PanelMsg::FpEditorSetNextPadSizeY(bad.into()));
        panel(&mut app, PanelMsg::FpEditorSetNextPadRotation(bad.into()));

        let after = &editor(&app, &path).state.next_pad_defaults;
        assert_eq!(after.size_x_mm, before.size_x_mm, "size_x took {bad:?}");
        assert_eq!(after.size_y_mm, before.size_y_mm, "size_y took {bad:?}");
        assert_eq!(
            after.rotation_deg, before.rotation_deg,
            "rotation took {bad:?}"
        );
    }
}

#[test]
fn selected_pad_fields_refuse_non_finite_values() {
    for bad in BAD_VALUES {
        let (mut app, path) = fixture();
        let before = editor(&app, &path).state.pads[0].clone();

        panel(
            &mut app,
            PanelMsg::FpEditorSetSelectedPadSizeX {
                idx: 0,
                value: bad.into(),
            },
        );
        panel(
            &mut app,
            PanelMsg::FpEditorSetSelectedPadSizeY {
                idx: 0,
                value: bad.into(),
            },
        );
        panel(
            &mut app,
            PanelMsg::FpEditorSetSelectedPadRotation {
                idx: 0,
                value: bad.into(),
            },
        );
        panel(
            &mut app,
            PanelMsg::FpEditorSetSelectedPadDrillDiameter {
                idx: 0,
                value: bad.into(),
            },
        );

        let after = &editor(&app, &path).state.pads[0];
        assert_eq!(after.size_mm, before.size_mm, "size took {bad:?}");
        assert_eq!(
            after.rotation_deg, before.rotation_deg,
            "rotation took {bad:?}"
        );
        assert_eq!(
            after.drill_diameter_mm, before.drill_diameter_mm,
            "drill took {bad:?}"
        );
    }
}

#[test]
fn silk_fields_refuse_non_finite_values() {
    for bad in BAD_VALUES {
        let (mut app, path) = fixture();

        panel(&mut app, PanelMsg::FpEditorSetSilkStrokeWidth(bad.into()));
        panel(&mut app, PanelMsg::FpEditorSetSilkLineFromX(bad.into()));
        panel(&mut app, PanelMsg::FpEditorSetSilkLineToY(bad.into()));

        let graphic = &editor(&app, &path).primitive().silk_f[0];
        assert_eq!(graphic.stroke_width, 0.1, "stroke took {bad:?}");
        let FpGraphicKind::Line { from, to } = &graphic.kind else {
            panic!("fixture silk is a line");
        };
        assert_eq!(*from, [0.0, 0.0], "line start took {bad:?}");
        assert_eq!(*to, [1.0, 0.0], "line end took {bad:?}");
    }
}

#[test]
fn a_refused_non_finite_edit_reaches_the_messages_panel() {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _path) = fixture();

    panel(&mut app, PanelMsg::FpEditorSetNextPadRotation("nan".into()));

    let reported = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|entry| {
            entry.message.contains("not a finite number") && entry.message.contains("nan")
        });
    assert!(
        reported,
        "the refusal must be logged for the Messages panel"
    );
}
