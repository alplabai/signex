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
/// blur that comes first commits the typed text.
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

fn multiplier(app: &mut Signex) -> u32 {
    app.active_footprint_editor_mut()
        .expect("footprint editor is active")
        .state
        .snap_options
        .coarse_multiplier
}

/// Cancel after typing on a preview label closes the dialog in one click,
/// whether the text is a length or not; nothing is written.
#[test]
fn cancel_after_typing_on_a_preview_label_closes_the_dialog() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    for typed in ["2.5", "abc"] {
        let mut app = app_with_footprint_editor();
        let before = step_mm(&mut app);
        send(&mut app, GridPropertiesMsg::Open);
        label(&mut app, M::LabelPressed(0));
        label(&mut app, M::DraftEdited(typed.into()));
        label(&mut app, M::DraftBlurred(0));
        send(&mut app, GridPropertiesMsg::CancelPressed);
        assert!(app.ui_state.grid_properties.is_none(), "typed {typed:?}");
        assert_eq!(step_mm(&mut app), before, "typed {typed:?}");
    }
}

/// A length typed on a preview label is committed when the label loses
/// focus, so a value typed afterwards in the Step field is what OK writes.
#[test]
fn a_step_typed_in_the_field_after_a_label_is_what_ok_writes() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    label(&mut app, M::DraftBlurred(0));
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert!(state.preview_labels.editing_index().is_none());
    send(&mut app, GridPropertiesMsg::SetStep("0.5".into()));
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 0.5);
    assert!(app.ui_state.grid_properties.is_none());
}

/// A refused label left open by its blur is dropped when the user edits
/// the Step field or picks a Multiplier: the value set last wins.
#[test]
fn an_edit_in_the_form_drops_a_refused_label() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("abc".into()));
    label(&mut app, M::DraftBlurred(0));
    send(&mut app, GridPropertiesMsg::SetStep("0.5".into()));
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert!(state.preview_labels.editing_index().is_none());

    label(&mut app, M::LabelPressed(1));
    label(&mut app, M::DraftEdited("abc".into()));
    label(&mut app, M::DraftBlurred(1));
    send(&mut app, GridPropertiesMsg::SetMultiplier(2));
    send(&mut app, GridPropertiesMsg::Apply);
    assert!(app.ui_state.grid_properties.is_none());
    assert_eq!(step_mm(&mut app), 0.5);
    assert_eq!(multiplier(&mut app), 2);
}

/// A press on the other label that arrives before the open field's blur
/// still commits the length typed on the open label.
#[test]
fn a_length_typed_on_one_label_is_kept_when_the_other_label_is_pressed() {
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    send(&mut app, GridPropertiesMsg::Open);
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    label(&mut app, M::LabelPressed(1));
    let state = app.ui_state.grid_properties.as_ref().expect("open");
    assert_eq!(state.preview_labels.editing_index(), Some(1));
    send(&mut app, GridPropertiesMsg::Close);
    send(&mut app, GridPropertiesMsg::Apply);
    assert_eq!(step_mm(&mut app), 2.5);
}

/// The OS close of a detached Grid Properties window closes the dialog in
/// one click, as Cancel does, even with a preview label open.
#[test]
fn the_os_close_of_the_detached_window_closes_the_dialog_with_a_label_open() {
    use crate::app::state::{ModalId, WindowKind};
    use crate::app::{Message, WindowMsg};
    use signex_widgets::dimension_label::DimensionLabelMessage as M;
    let mut app = app_with_footprint_editor();
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    app.ui_state.main_window_opened = true;
    app.ui_state.first_run_tour_open = false;
    send(&mut app, GridPropertiesMsg::Open);
    let _ = app.sync_detached_modals();
    let window = app
        .ui_state
        .windows
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, WindowKind::DetachedModal(ModalId::GridProperties)).then_some(*id)
        })
        .expect("the dialog has its own window");
    label(&mut app, M::LabelPressed(0));
    label(&mut app, M::DraftEdited("2.5".into()));
    let _ = app.update(Message::Window(WindowMsg::WindowCloseRequested(window)));
    assert!(app.ui_state.grid_properties.is_none());
}
