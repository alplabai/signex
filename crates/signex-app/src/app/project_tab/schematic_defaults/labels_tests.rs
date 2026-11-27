//! Mockup row 36, #2685: values edited on the Schematic defaults drawing are
//! the field's own edit and one undo step; Tab, Esc and the handles. The
//! larger preview window of rows 31 and 36.

use iced::Point;
use signex_types::project::SchematicDefaultField as Field;
use signex_widgets::dimension_label::DimensionLabelMessage as M;

use super::labels::preview_labels;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::project_tab::{PreviewPopup, ProjectPreviewMsg, ProjectSection};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-labels-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::SectionSelected {
            project_id: id,
            section: ProjectSection::SchematicDefaults,
        },
    )));
    (app, id)
}

fn preview(app: &mut Signex, id: ProjectId, msg: ProjectPreviewMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::PreviewActed {
            project_id: id,
            msg,
        },
    )));
}

fn label(app: &mut Signex, id: ProjectId, msg: M) {
    preview(app, id, ProjectPreviewMsg::Labels(msg));
}

fn index(field: Field) -> usize {
    Field::ALL
        .iter()
        .position(|f| *f == field)
        .expect("a listed field")
}

fn stored(app: &Signex, field: Field) -> Option<f64> {
    app.document_state.projects[0]
        .data
        .schematic_defaults
        .get(field)
}

fn open_with(app: &mut Signex, id: ProjectId, field: Field, typed: &str) {
    label(app, id, M::LabelPressed(index(field)));
    label(app, id, M::DraftEdited(typed.to_string()));
}

#[test]
fn enter_commits_the_value_as_one_undo_step() {
    let (mut app, id) = app();
    open_with(&mut app, id, Field::WireWidth, "0.3");
    label(&mut app, id, M::DraftSubmitted);
    assert_eq!(stored(&app, Field::WireWidth), Some(0.3));
    assert!(
        app.ui_state
            .project_preview
            .labels_of(id)
            .editing_index()
            .is_none()
    );
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(stored(&app, Field::WireWidth), None);
}

#[test]
fn a_refused_value_stays_open_and_shows_in_the_field() {
    let (mut app, id) = app();
    open_with(&mut app, id, Field::JunctionDiameter, "5");
    label(&mut app, id, M::DraftSubmitted);
    assert_eq!(stored(&app, Field::JunctionDiameter), None);
    let ui = &app.ui_state.project_preview;
    assert_eq!(
        ui.labels_of(id).editing_index(),
        Some(index(Field::JunctionDiameter))
    );
    let draft = app.document_view.project_tabs[&id]
        .default_drafts
        .get(&Field::JunctionDiameter);
    assert_eq!(draft.map(String::as_str), Some("5"));
    assert_eq!(
        super::refused_range(Field::JunctionDiameter, draft),
        Some((0.2, 2.0))
    );
}

#[test]
fn esc_cancels_and_takes_a_refused_text_back_out_of_the_field() {
    let (mut app, id) = app();
    open_with(&mut app, id, Field::JunctionDiameter, "5");
    label(&mut app, id, M::DraftSubmitted);
    label(&mut app, id, M::EscapePressed);
    assert!(
        app.ui_state
            .project_preview
            .labels_of(id)
            .editing_index()
            .is_none()
    );
    assert!(
        !app.document_view.project_tabs[&id]
            .default_drafts
            .contains_key(&Field::JunctionDiameter)
    );
    assert_eq!(stored(&app, Field::JunctionDiameter), None);
}

#[test]
fn tab_commits_and_opens_the_next_value() {
    let (mut app, id) = app();
    let first = Field::ALL[0];
    open_with(&mut app, id, first, "0.4");
    label(&mut app, id, M::TabPressed { backward: false });
    assert_eq!(stored(&app, first), Some(0.4));
    assert_eq!(
        app.ui_state.project_preview.labels_of(id).editing_index(),
        Some(1)
    );
}

#[test]
fn an_emptied_label_returns_to_the_built_in_value() {
    let (mut app, id) = app();
    open_with(&mut app, id, Field::BusWidth, "0.9");
    label(&mut app, id, M::DraftSubmitted);
    assert_eq!(stored(&app, Field::BusWidth), Some(0.9));
    open_with(&mut app, id, Field::BusWidth, "");
    label(&mut app, id, M::DraftSubmitted);
    assert_eq!(stored(&app, Field::BusWidth), None);
}

#[test]
fn a_label_opened_and_left_writes_nothing() {
    let (mut app, id) = app();
    label(&mut app, id, M::LabelPressed(index(Field::WireWidth)));
    label(&mut app, id, M::DraftSubmitted);
    assert_eq!(stored(&app, Field::WireWidth), None);
    assert!(!app.step_project_tab_history(id, true));
}

/// A drag of the bus handle writes as it goes and is one undo step.
#[test]
fn a_handle_drag_is_one_undo_step() {
    let (mut app, id) = app();
    let bus = index(Field::BusWidth);
    label(&mut app, id, M::HandlePressed(bus));
    label(&mut app, id, M::PointerMoved(Point::new(500.0, 40.0)));
    label(&mut app, id, M::PointerMoved(Point::new(500.0, 30.0)));
    label(&mut app, id, M::PointerMoved(Point::new(500.0, 20.0)));
    label(&mut app, id, M::PointerReleased);
    let dragged = stored(&app, Field::BusWidth).expect("the drag wrote the bus width");
    assert!(Field::BusWidth.range().contains(&dragged));
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(stored(&app, Field::BusWidth), None);
    assert!(!app.step_project_tab_history(id, true));
}

/// Esc during a drag puts back the unset default, not its resolved value.
#[test]
fn esc_during_a_drag_restores_an_unset_default() {
    let (mut app, id) = app();
    let junction = index(Field::JunctionDiameter);
    label(&mut app, id, M::HandlePressed(junction));
    label(&mut app, id, M::PointerMoved(Point::new(80.0, 110.0)));
    label(&mut app, id, M::PointerMoved(Point::new(90.0, 120.0)));
    assert!(stored(&app, Field::JunctionDiameter).is_some());
    label(&mut app, id, M::EscapePressed);
    assert_eq!(stored(&app, Field::JunctionDiameter), None);
}

/// Only the junction and the bus have handles, and a label's order is the
/// field order the commits are read back through.
#[test]
fn the_junction_and_the_bus_carry_the_handles() {
    let overrides = signex_types::project::SchematicDefaultOverrides::default();
    let i18n = signex_i18n::I18n::en_us();
    let shown = preview_labels(&overrides, 1.0, &i18n);
    assert_eq!(shown.len(), Field::ALL.len());
    for (field, shown) in Field::ALL.iter().zip(&shown) {
        let wants = matches!(field, Field::JunctionDiameter | Field::BusWidth);
        assert_eq!(shown.handle.is_some(), wants, "{field:?}");
    }
}

#[test]
fn the_larger_window_opens_renders_and_closes_on_esc() {
    let (mut app, id) = app();
    let window = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(window);
    for popup in [PreviewPopup::SchematicDefaults, PreviewPopup::Sheet] {
        preview(&mut app, id, ProjectPreviewMsg::PopupOpened(popup));
        assert_eq!(app.ui_state.project_preview.popup_of(id), Some(popup));
        let _ = app.view_project_tab(id);
        let esc = iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            modified_key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        };
        let message = app
            .project_preview_key(window, &esc)
            .expect("Esc closes the window");
        let _ = app.update(message);
        assert_eq!(app.ui_state.project_preview.popup_of(id), None);
    }
}
