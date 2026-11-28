//! #1282: the Differential Pair Classes dialog edits a copy; OK applies it
//! as one project step, Cancel drops it.

use std::collections::HashMap;

use iced::keyboard::{Key, Modifiers, key::Named};
use signex_types::diff_pair_class::DiffPairClass;
use signex_types::project::ProjectData;
use signex_widgets::dimension_label::DimensionLabelMessage as L;

use super::preview::{PairGeometry, PairPreviewMsg, preview_labels, value_texts};
use super::{
    DiffPairClassesMsg, DiffPairClassesState, PairClassField, PairClassProblem, PairMemberRow,
    PairPreview, free_name, new_class,
};
use crate::app::Signex;
use crate::app::state::{LoadedProject, ProjectId};

const PROJECT: ProjectId = ProjectId::test_id(41);

fn class(name: &str) -> DiffPairClass {
    new_class(name.to_string())
}

fn state(names: &[&str], members: &[(&str, &str)]) -> DiffPairClassesState {
    DiffPairClassesState {
        project: PROJECT,
        project_name: "dcdc-buck".to_string(),
        classes: names.iter().map(|n| class(n)).collect(),
        original_names: names.iter().map(|n| Some(n.to_string())).collect(),
        has_closed_sheets: false,
        selected: 0,
        menu: None,
        members: members
            .iter()
            .map(|(class, pair)| PairMemberRow {
                class: class.to_string(),
                pair: pair.to_string(),
                positive: format!("{pair}_P"),
                negative: format!("{pair}_N"),
                sheet: "power_in.snxsch".to_string(),
            })
            .collect(),
        drafts: Vec::new(),
        problem: None,
        preview: PairPreview::default(),
    }
}

#[test]
fn a_new_name_is_free_and_a_class_with_pairs_cannot_be_deleted() {
    let s = state(&["USB", "USB_2"], &[("USB", "USB_D")]);
    assert_eq!(free_name(&s.classes, "USB"), "USB_3");
    assert_eq!(free_name(&s.classes, "HDMI"), "HDMI");
    assert_eq!(s.pair_count(0), 1);
    assert!(!s.can_delete(0));
    assert!(s.can_delete(1));
    assert!(!s.can_delete(7));
    assert!(class("X").pair_by_suffix, "a new class pairs by suffix");
}

#[test]
fn ok_is_refused_for_a_blank_or_repeated_name() {
    let mut s = state(&["USB", " USB "], &[]);
    assert_eq!(s.problem(), Some(PairClassProblem::DuplicateName));
    s.classes[1].name = "  ".to_string();
    assert_eq!(s.problem(), Some(PairClassProblem::BlankName));
    s.classes[1].name = " HDMI ".to_string();
    assert_eq!(s.problem(), None);
    assert_eq!(s.applied()[1].name, "HDMI");
}

fn app_with_project(classes: Vec<DiffPairClass>) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut data: ProjectData = serde_json::from_str(
        r#"{"name":"dcdc-buck","dir":"","schematic_root":null,"pcb_file":null}"#,
    )
    .expect("a project");
    data.schematic_design.diff_pair_classes = classes;
    app.document_state.projects.push(LoadedProject {
        id: PROJECT,
        path: std::env::temp_dir().join("dpc-dialog.snxprj"),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app
}

fn classes_of(app: &Signex) -> Vec<DiffPairClass> {
    app.document_state
        .project_by_id(PROJECT)
        .expect("the project")
        .data
        .schematic_design
        .diff_pair_classes
        .clone()
}

/// Open the dialog on the test project directly: no document is in view.
fn open(app: &mut Signex) {
    let classes = classes_of(app);
    let original_names = classes.iter().map(|c| Some(c.name.clone())).collect();
    app.ui_state.diff_pair_classes = Some(Box::new(DiffPairClassesState {
        classes,
        original_names,
        ..state(&[], &[])
    }));
}

/// DECISIONS 2026-11-23 (#2750): OK over an unreadable section changes
/// nothing and keeps the dialog open.
#[test]
fn ok_over_an_unreadable_section_is_refused() {
    let mut app = app_with_project(Vec::new());
    let garbled = serde_json::json!("garbled");
    let text = br#"{"name":"dcdc-buck","dir":"","schematic_root":null,"pcb_file":null,"schematic_design":"garbled"}"#;
    app.document_state.projects[0].data = signex_types::project::parse_project_bytes(
        std::path::Path::new("dcdc-buck.snxprj"),
        text,
        signex_types::project::files_on_disk(std::path::Path::new("dcdc-buck.snxprj")),
    )
    .expect("a project");
    assert!(
        app.document_state.projects[0]
            .data
            .has_unreadable_schematic_design()
    );
    open(&mut app);
    let _ = app.update(DiffPairClassesMsg::ClassAdded.into());
    let _ = app.update(DiffPairClassesMsg::OkRequested.into());
    assert!(classes_of(&app).is_empty());
    assert!(app.ui_state.diff_pair_classes.is_some(), "the dialog stays");
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.unknown.get("schematic_design"), Some(&garbled));
}

fn dialog(app: &Signex) -> &DiffPairClassesState {
    app.ui_state.diff_pair_classes.as_deref().expect("open")
}

/// Directives name a class, and a rename is not carried to them: a class
/// with pairs keeps its name, so renaming it cannot slip past the guard
/// that keeps it from being deleted.
#[test]
fn a_class_with_pairs_can_be_neither_renamed_nor_then_deleted() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.members = state(&[], &[("USB", "USB_D")]).members;
    }
    send(&mut app, DiffPairClassesMsg::NameEdited("USB3".into()));
    assert_eq!(dialog(&app).classes[0].name, "USB", "the name is kept");
    assert_eq!(dialog(&app).pair_count(0), 1);
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    assert_eq!(dialog(&app).classes.len(), 1, "and it is not deleted");
    send(&mut app, DiffPairClassesMsg::OkRequested);
    assert_eq!(classes_of(&app), [class("USB")]);
}

/// The pair count follows the name the class had when the dialog opened.
#[test]
fn pairs_are_counted_by_the_original_name() {
    let mut s = state(&["USB", "HDMI"], &[("USB", "USB_D")]);
    s.classes[0].name = "renamed".to_string();
    assert_eq!(s.pair_count(0), 1);
    assert!(!s.can_delete(0));
}

/// Pairs on sheets that are not open are unknown, so no class that was in
/// the project may be renamed or deleted; one added here may.
#[test]
fn closed_sheets_lock_every_existing_class() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.has_closed_sheets = true;
    }
    assert_eq!(dialog(&app).lock(0), Some(super::ClassLock::SheetsClosed));
    send(&mut app, DiffPairClassesMsg::NameEdited("USB3".into()));
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    assert_eq!(dialog(&app).classes, [class("USB")]);
    send(&mut app, DiffPairClassesMsg::ClassAdded);
    assert_eq!(dialog(&app).classes[1].name, "DIFF");
    assert_eq!(dialog(&app).lock(1), None, "a new class is free");
    send(&mut app, DiffPairClassesMsg::ClassDeleted(1));
    assert_eq!(dialog(&app).classes.len(), 1);
}

fn send(app: &mut Signex, msg: DiffPairClassesMsg) {
    let _ = app.dispatch_diff_pair_classes_message(msg);
}

#[test]
fn ok_applies_every_edit_as_one_undo_step() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Width, "0.2".into()),
    );
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Gap, "0.15".into()),
    );
    send(&mut app, DiffPairClassesMsg::ClassAdded);
    send(&mut app, DiffPairClassesMsg::NameEdited("HDMI".into()));
    assert_eq!(
        classes_of(&app),
        [class("USB")],
        "nothing applied before OK"
    );

    send(&mut app, DiffPairClassesMsg::OkRequested);
    assert!(app.ui_state.diff_pair_classes.is_none(), "OK closes");
    let applied = classes_of(&app);
    assert_eq!(applied.len(), 2);
    assert_eq!(applied[0].width_mm, Some(0.2));
    assert_eq!(applied[0].gap_mm, Some(0.15));
    assert_eq!(applied[1].name, "HDMI");

    let document = &mut app.document_state;
    let project = document
        .projects
        .iter_mut()
        .find(|p| p.id == PROJECT)
        .unwrap();
    let history = document
        .general_history
        .get_mut(&PROJECT)
        .expect("a history");
    assert!(history.undo(&mut project.data));
    assert_eq!(
        project.data.schematic_design.diff_pair_classes,
        [class("USB")],
        "one undo step puts every edit back"
    );
    assert!(!history.can_undo());
}

/// #2975 — the General history is the project's, not the tab's: a step
/// recorded with no Project tab state still undoes through the tab path.
#[test]
fn project_undo_steps_an_edit_made_without_a_project_tab() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    send(&mut app, DiffPairClassesMsg::OkRequested);
    assert!(classes_of(&app).is_empty());
    app.document_view.project_tabs.remove(&PROJECT);

    assert!(app.step_project_tab_history(PROJECT, true));
    assert_eq!(classes_of(&app), [class("USB")]);
}

#[test]
fn cancel_changes_nothing() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    send(&mut app, DiffPairClassesMsg::PairBySuffixToggled(false));
    send(&mut app, DiffPairClassesMsg::CancelRequested);
    assert!(app.ui_state.diff_pair_classes.is_none());
    assert_eq!(classes_of(&app), [class("USB")]);
    assert!(
        app.document_state
            .general_history
            .get(&PROJECT)
            .is_none_or(|history| !history.can_undo()),
        "no step recorded"
    );
}

#[test]
fn a_typed_value_is_kept_while_typed_and_a_class_with_pairs_stays() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    if let Some(s) = app.ui_state.diff_pair_classes.as_mut() {
        s.members = state(&[], &[("USB", "USB_D")]).members;
    }
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Impedance, "9".into()),
    );
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Impedance, "90.".into()),
    );
    send(&mut app, DiffPairClassesMsg::ClassDeleted(0));
    let s = app.ui_state.diff_pair_classes.as_deref().unwrap();
    assert_eq!(s.draft(PairClassField::Impedance), Some("90."));
    assert_eq!(s.classes[0].impedance_ohm, Some(90.0));
    assert_eq!(s.classes.len(), 1, "a class with pairs is not deleted");
}

// #2661, mockup row 153: the values on the class preview are edited there.

fn on_preview(app: &mut Signex, message: L) {
    send(
        app,
        DiffPairClassesMsg::PreviewActed(PairPreviewMsg::Labels(message)),
    );
}

/// Esc as the key routing hands it to the preview.
fn press_escape(app: &mut Signex) {
    let esc = dialog(app)
        .preview
        .key_message(&Key::Named(Named::Escape), Modifiers::empty())
        .expect("Esc belongs to the preview");
    send(app, DiffPairClassesMsg::PreviewActed(esc));
}

#[test]
fn a_value_typed_on_the_preview_is_the_fields_edit() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    on_preview(&mut app, L::LabelPressed(0));
    on_preview(&mut app, L::DraftEdited("0.25".into()));
    on_preview(&mut app, L::DraftSubmitted);
    assert_eq!(dialog(&app).classes[0].width_mm, Some(0.25));
    assert_eq!(dialog(&app).draft(PairClassField::Width), Some("0.25"));

    // Tab commits the impedance and opens its tolerance.
    on_preview(&mut app, L::LabelPressed(2));
    on_preview(&mut app, L::DraftEdited("90".into()));
    on_preview(&mut app, L::TabPressed { backward: false });
    assert_eq!(dialog(&app).classes[0].impedance_ohm, Some(90.0));
    assert_eq!(dialog(&app).preview.labels.editing_index(), Some(3));
    on_preview(&mut app, L::DraftEdited("10".into()));
    on_preview(&mut app, L::DraftSubmitted);
    assert_eq!(dialog(&app).classes[0].impedance_tolerance_pct, Some(10.0));
    assert_eq!(
        classes_of(&app),
        [class("USB")],
        "nothing applied before OK"
    );

    send(&mut app, DiffPairClassesMsg::OkRequested);
    let applied = classes_of(&app);
    assert_eq!(applied[0].width_mm, Some(0.25));
    assert_eq!(applied[0].impedance_ohm, Some(90.0));
    assert_eq!(applied[0].impedance_tolerance_pct, Some(10.0));
}

/// #2661, done when: edits on the preview, typed and dragged, apply on OK
/// and undo as one step.
#[test]
fn preview_edits_apply_on_ok_and_undo_as_one_step() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    on_preview(&mut app, L::LabelPressed(0));
    on_preview(&mut app, L::DraftEdited("0.25".into()));
    on_preview(&mut app, L::DraftSubmitted);
    on_preview(&mut app, L::HandlePressed(1));
    on_preview(&mut app, L::PointerMoved(iced::Point::ORIGIN));
    on_preview(&mut app, L::PointerMoved(iced::Point::new(0.0, 20.0)));
    on_preview(&mut app, L::PointerReleased);
    let gap = dialog(&app).classes[0].gap_mm.expect("a dragged gap");
    assert_eq!(
        classes_of(&app),
        [class("USB")],
        "nothing applied before OK"
    );

    send(&mut app, DiffPairClassesMsg::OkRequested);
    let applied = classes_of(&app);
    assert_eq!(applied[0].width_mm, Some(0.25));
    assert_eq!(applied[0].gap_mm, Some(gap));

    let document = &mut app.document_state;
    let project = document
        .projects
        .iter_mut()
        .find(|p| p.id == PROJECT)
        .unwrap();
    let history = document
        .general_history
        .get_mut(&PROJECT)
        .expect("a history");
    assert!(history.undo(&mut project.data));
    assert_eq!(
        project.data.schematic_design.diff_pair_classes,
        [class("USB")],
        "one undo step puts the preview edits back"
    );
    assert!(!history.can_undo());
}

/// #2661, done when: the preview draws the dialog's copy while it is
/// edited, before OK applies it.
#[test]
fn the_preview_shows_a_field_edit_before_ok() {
    let usb = DiffPairClass {
        width_mm: Some(0.2),
        gap_mm: Some(0.15),
        ..class("USB")
    };
    let mut app = app_with_project(vec![usb.clone()]);
    open(&mut app);
    let geometry = |app: &Signex| {
        let s = dialog(app);
        PairGeometry::of(s.selected_class().expect("a class"), &s.preview)
    };
    let before = geometry(&app);
    send(
        &mut app,
        DiffPairClassesMsg::FieldEdited(PairClassField::Width, "0.3".into()),
    );
    let after = geometry(&app);
    assert!(after.trace_w > before.trace_w, "the traces are drawn wider");
    let en = signex_i18n::I18n::en_us();
    let [width, ..] = value_texts(dialog(&app).selected_class().expect("a class"), &en);
    assert!(
        width.into_string().contains("0.3"),
        "the drawing writes 0.3"
    );
    assert_eq!(classes_of(&app), [usb], "nothing applied before OK");
}

/// An unset value opens at what the drawing shows; left as it opened, it
/// stays unset.
#[test]
fn an_unset_value_opened_and_left_stays_unset() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    on_preview(&mut app, L::LabelPressed(1));
    on_preview(&mut app, L::DraftSubmitted);
    assert_eq!(dialog(&app).classes[0].gap_mm, None);
    assert_eq!(dialog(&app).draft(PairClassField::Gap), None);
}

/// Esc closes the open value only: the dialog stays and the value is kept.
#[test]
fn esc_closes_an_open_value_and_keeps_the_dialog() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    on_preview(&mut app, L::LabelPressed(0));
    on_preview(&mut app, L::DraftEdited("0.9".into()));
    press_escape(&mut app);
    assert!(app.ui_state.diff_pair_classes.is_some(), "the dialog stays");
    assert_eq!(dialog(&app).classes[0].width_mm, None);
    assert!(dialog(&app).preview.labels.editing_index().is_none());
    let next = dialog(&app)
        .preview
        .key_message(&Key::Named(Named::Escape), Modifiers::empty());
    assert!(next.is_none(), "the next Esc is the dialog's");
}

/// The N trace follows the pointer at the scale the drawing had when the
/// handle was pressed; Esc puts the gap back and drops the draft.
#[test]
fn the_handle_drags_the_gap_and_esc_puts_it_back() {
    let wide = DiffPairClass {
        width_mm: Some(0.2),
        gap_mm: Some(0.15),
        ..class("USB")
    };
    let mut app = app_with_project(vec![wide]);
    open(&mut app);
    let scale = PairGeometry::of(&dialog(&app).classes[0], &dialog(&app).preview).scale;
    on_preview(&mut app, L::HandlePressed(1));
    on_preview(&mut app, L::PointerMoved(iced::Point::ORIGIN));
    // 0.05 mm more gap moves the N trace 0.025 mm down.
    let dy = (0.025 * scale) as f32 * PairGeometry::unit_px();
    on_preview(&mut app, L::PointerMoved(iced::Point::new(0.0, dy)));
    let gap = dialog(&app).classes[0].gap_mm.expect("a gap");
    assert!((gap - 0.2).abs() < 1e-9, "{gap}");
    assert_eq!(dialog(&app).draft(PairClassField::Gap), Some("0.2"));
    let held = PairGeometry::of(&dialog(&app).classes[0], &dialog(&app).preview).scale;
    assert_eq!(held, scale, "the drawing keeps its scale under the pointer");

    press_escape(&mut app);
    assert_eq!(dialog(&app).classes[0].gap_mm, Some(0.15));
    assert_eq!(dialog(&app).draft(PairClassField::Gap), None);
    assert!(dialog(&app).preview.labels.dragging_index().is_none());
}

/// A gap dragged on a class with none, then let go, is the class's gap.
#[test]
fn a_released_handle_keeps_the_dragged_gap() {
    let mut app = app_with_project(vec![class("USB")]);
    open(&mut app);
    on_preview(&mut app, L::HandlePressed(1));
    on_preview(&mut app, L::PointerMoved(iced::Point::ORIGIN));
    on_preview(&mut app, L::PointerMoved(iced::Point::new(0.0, 40.0)));
    on_preview(&mut app, L::PointerReleased);
    let gap = dialog(&app).classes[0].gap_mm.expect("a gap");
    assert!(gap > super::preview::DRAWN_GAP, "{gap}");
    assert!(dialog(&app).preview.labels.dragging_index().is_none());
    let free = PairGeometry::of(&dialog(&app).classes[0], &dialog(&app).preview);
    assert!(
        free.y_n + free.trace_w / 2.0 < free.view_h,
        "the drag kept the N trace inside the drawing"
    );
}

#[test]
fn selecting_another_class_closes_an_open_value_and_expand_is_kept() {
    let mut app = app_with_project(vec![class("USB"), class("HDMI")]);
    open(&mut app);
    send(
        &mut app,
        DiffPairClassesMsg::PreviewActed(PairPreviewMsg::ExpandToggled),
    );
    on_preview(&mut app, L::LabelPressed(0));
    send(&mut app, DiffPairClassesMsg::ClassSelected(1));
    assert!(dialog(&app).preview.labels.editing_index().is_none());
    assert!(dialog(&app).preview.expanded);
    send(
        &mut app,
        DiffPairClassesMsg::PreviewActed(PairPreviewMsg::ExpandToggled),
    );
    assert!(!dialog(&app).preview.expanded);
}

/// Every value is a press target over the text the drawing writes; only
/// the gap has a handle, and the tolerance follows the impedance.
#[test]
fn the_preview_labels_are_the_four_values_and_one_handle() {
    let en = signex_i18n::I18n::en_us();
    let labels = preview_labels(&class("USB"), &PairPreview::default(), &en);
    let handles: Vec<bool> = labels.iter().map(|l| l.handle.is_some()).collect();
    assert_eq!(handles, [false, true, false, false]);
    assert!(labels.iter().all(|l| l.is_bare));
    assert!(labels[3].at.x > labels[2].at.x);
    assert_eq!(labels[2].at.y, labels[3].at.y);
}
