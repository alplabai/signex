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

fn label(app: &mut Signex, m: signex_widgets::dimension_label::DimensionLabelMessage) {
    send(app, GridPropertiesMsg::PreviewLabelEdited(m));
}

/// A step typed on the preview's label is the step OK writes.
#[test]
fn a_step_set_on_the_preview_is_what_ok_writes() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    label(&mut app, M::DraftSubmitted);
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
}

/// OK with a preview label still open commits its draft, as Enter would.
#[test]
fn ok_commits_an_open_preview_label_draft() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
    assert!(app.ui_state.grid_properties.is_none());
}

/// Esc with a preview label open closes the label; the next Esc closes
/// the dialog.
#[test]
fn escape_closes_an_open_preview_label_before_the_dialog() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    send(&mut app, GridPropertiesMsg::Close);
    let state = app.ui_state.grid_properties.as_ref().expect("still open");
    assert!(state.preview_labels.editing_index().is_none());
    send(&mut app, GridPropertiesMsg::Close);
    assert!(app.ui_state.grid_properties.is_none());
}

/// The expand button toggles a taller preview.
#[test]
fn the_expand_button_toggles_a_taller_preview() {
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    let expanded = |app: &Signex| {
        app.ui_state
            .grid_properties
            .as_ref()
            .is_some_and(|s| s.preview_expanded)
    };
    assert!(!expanded(&app));
    send(&mut app, GridPropertiesMsg::PreviewExpandToggled);
    assert!(expanded(&app));
    send(&mut app, GridPropertiesMsg::PreviewExpandToggled);
    assert!(!expanded(&app));
}

/// OK with a preview label holding text that is not a length is refused:
/// the dialog and the label stay open with the text, and the step is not
/// written. Once the text is a length, OK commits it. Each OK is a mouse
/// click, so the field's blur arrives first.
#[test]
fn ok_is_refused_while_a_preview_label_holds_text_that_is_not_a_length() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    let before = step_mm(&mut app);
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("abc".into()));
    label(&mut app, M::DraftBlurred(0));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), before);
    let state = app.ui_state.grid_properties.as_ref().expect("still open");
    assert_eq!(state.preview_labels.editing_index(), Some(0));
    assert_eq!(state.preview_labels.draft(), Some("abc"));

    label(&mut app, M::DraftEdited("2.5".into()));
    label(&mut app, M::DraftBlurred(0));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
    assert!(app.ui_state.grid_properties.is_none());
}

/// A click on OK after typing a length on a fresh label writes it: the
/// blur that comes first keeps the typed text.
#[test]
fn ok_clicked_after_typing_a_length_writes_it() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    label(&mut app, M::DraftBlurred(0));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
    assert!(app.ui_state.grid_properties.is_none());
}

/// A blur on a label whose text is untouched still closes it, and a
/// refused draft is not replaced by a press on the other label.
#[test]
fn a_refused_draft_stays_when_the_other_label_is_pressed() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert_eq!(state.preview_label_values().len(), 2, "step and coarse");
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftBlurred(0));
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert!(state.preview_labels.editing_index().is_none());

    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("abc".into()));
    label(&mut app, M::DraftBlurred(0));
    label(&mut app, M::LabelPressed(1));
    label(&mut app, M::HandlePressed(1));
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert_eq!(state.preview_labels.editing_index(), Some(0));
    assert_eq!(state.preview_labels.draft(), Some("abc"));

    send(&mut app, GridPropertiesMsg::Close);
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert!(state.preview_labels.editing_index().is_none());
}
