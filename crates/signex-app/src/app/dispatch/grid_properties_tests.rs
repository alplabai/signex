//! Grid Properties has one Step field: `SnapOptions` stores a single grid
//! step for both axes, so there is no second axis input to drop on Apply.

use std::path::PathBuf;

use crate::app::{FootprintDocument, GridPropertiesMsg, Signex, TabInfo, TabKind};

fn app_with_footprint_editor() -> Signex {
    let (mut app, _boot) = Signex::new();
    let path = PathBuf::from("/w/part.snxfpt");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "part".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = 0;
    app
}

fn send(app: &mut Signex, msg: GridPropertiesMsg) {
    let _ = app.dispatch_grid_properties_message(msg);
}

fn step_mm(app: &mut Signex) -> f64 {
    app.active_footprint_editor_mut()
        .expect("footprint editor is active")
        .state
        .snap_options
        .grid_step_mm
}

#[test]
fn apply_writes_the_step_and_closes_the_dialog() {
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    send(&mut app, GridPropertiesMsg::SetStep("2.5".into()));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
    assert!(app.ui_state.grid_properties.is_none());
}

#[test]
fn an_invalid_step_keeps_the_old_value_and_the_dialog_open() {
    let mut app = app_with_footprint_editor();
    let before = step_mm(&mut app);
    send(&mut app, GridPropertiesMsg::Open);
    send(&mut app, GridPropertiesMsg::SetStep("0".into()));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), before);
    assert!(app.ui_state.grid_properties.is_some());
}
