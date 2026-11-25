//! #2868: a new dimension starts from the PCB Defaults of Preferences; the
//! Dimension tool's form overrides them for the session.

use signex_engine::pcb::primitive_edit::DimensionStyleEdit;
use signex_types::format::{SNXPCB_FORMAT_V1_11, SnxPcb, board_uses_dimension_style, snxpcb_token};
use signex_types::pcb::Point;
use signex_types::pcb_dimension_style::{
    ArrowStyle, DimensionDefaults, DimensionStyle, DimensionTextPosition, DimensionUnits,
};

use super::form_tests::{enter, panel, typed};
use super::tests::{app_with_board, board_of, click, run};
use crate::app::Signex;
use crate::panels::PcbPrimitiveMsg;
use crate::panels::pcb_primitive_properties::{PrimitiveField, PrimitiveTarget};

fn place(app: &mut Signex) {
    run(app, "place_board_dimension");
    click(app, 1.0, 3.0);
    click(app, 6.0, 3.0);
    click(app, 6.0, 5.0);
}

fn custom() -> DimensionDefaults {
    DimensionDefaults {
        line_width: 0.25,
        text_height: 1.5,
        precision: 1,
        style: DimensionStyle {
            arrow_style: ArrowStyle::Filled,
            arrow_length: 2.0,
            text_position: DimensionTextPosition::Inline,
            unit: DimensionUnits::Mil,
            suffix: " ref".into(),
            ..DimensionStyle::default()
        },
    }
}

/// Caner 2026-11-24: the project's own dimension defaults win over
/// Preferences, which only seed new projects.
#[test]
fn a_new_dimension_starts_from_its_projects_dimension_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.pcb_dimension_defaults = custom();
    let mut project = DimensionDefaults::default();
    project.style.arrow_style = ArrowStyle::None;
    project.precision = 3;
    app.document_state.projects[0]
        .data
        .project_defaults
        .dimension = Some(project.clone());
    assert_eq!(app.dimension_defaults_for(&path), project);
    place(&mut app);
    let placed = &board_of(&app, &path).free.dimensions[0];
    assert_eq!(placed.style, project.style);
    assert_eq!(placed.precision, 3);
    assert_eq!(placed.value_text(), "5.000mm");
}

/// A project that stores no dimension defaults (one from before) follows
/// Preferences.
#[test]
fn a_new_dimension_starts_from_the_pcb_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.pcb_dimension_defaults = custom();
    place(&mut app);
    let dims = &board_of(&app, &path).free.dimensions;
    assert_eq!(dims.len(), 1);
    let placed = &dims[0];
    assert_eq!(placed.line_width, 0.25);
    assert_eq!(placed.text_height, 1.5);
    assert_eq!(placed.precision, 1);
    assert_eq!(placed.style, custom().style);
    assert_eq!(placed.value_text(), "196.9mil ref");
}

#[test]
fn without_saved_defaults_a_new_dimension_has_the_old_look() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    place(&mut app);
    let placed = &board_of(&app, &path).free.dimensions[0];
    assert_eq!(placed.style, DimensionStyle::default());
    assert_eq!(placed.value_text(), "5.00mm");
}

/// The form of the armed tool sets what it places next; Preferences keep
/// their values.
#[test]
fn the_tools_form_overrides_the_defaults_for_the_next_dimension() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    app.ui_state.pcb_dimension_defaults = custom();
    run(&mut app, "place_board_dimension");
    panel(
        &mut app,
        PcbPrimitiveMsg::DimensionStylePicked {
            target: PrimitiveTarget::placement(path.clone()),
            edit: DimensionStyleEdit::ArrowStyle(ArrowStyle::None),
        },
    );
    typed(&mut app, &path, PrimitiveField::Prefix, "L=");
    enter(&mut app, &path, PrimitiveField::Prefix);
    click(&mut app, 1.0, 3.0);
    click(&mut app, 6.0, 3.0);
    click(&mut app, 6.0, 5.0);
    let placed = &board_of(&app, &path).free.dimensions[0];
    assert_eq!(placed.style.arrow_style, ArrowStyle::None);
    assert_eq!(placed.style.prefix, "L=");
    assert_eq!(
        placed.style.unit,
        DimensionUnits::Mil,
        "the rest are the defaults'"
    );
    assert_eq!(app.ui_state.pcb_dimension_defaults, custom());
}

/// The token a save of `board` writes on its first line, and the file.
fn saved_token(board: &signex_types::pcb::PcbBoard) -> (String, String) {
    let text = SnxPcb::new(board.clone()).write_string().expect("writes");
    let token = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("format = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_default()
        .to_string();
    (token, text)
}

/// #2868: a dimension the Dimension tool places with the default look puts
/// its value Above, its point off the line; the board still saves with the
/// token it had and re-saves byte-identical.
#[test]
fn a_placed_default_dimension_keeps_the_boards_token() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    let before = snxpcb_token(board_of(&app, &path));
    place(&mut app);
    let board = board_of(&app, &path).clone();
    assert_eq!(board.free.dimensions.len(), 1);
    assert!(!board_uses_dimension_style(&board));
    assert_eq!(snxpcb_token(&board), before);
    assert_ne!(snxpcb_token(&board), SNXPCB_FORMAT_V1_11);
    let (token, text) = saved_token(&board);
    assert_eq!(token, before);
    let back = SnxPcb::parse(&text).expect("reads").board;
    assert_eq!(back.free.dimensions, board.free.dimensions);
    assert_eq!(saved_token(&back).1, text, "a re-save is byte-identical");
}

/// A placed dimension made Manual with its text moved writes `snxpcb/1.11`
/// and reads its point back.
#[test]
fn a_moved_manual_dimension_raises_the_token_and_keeps_its_point() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, path) = app_with_board(dir.path());
    place(&mut app);
    let mut board = board_of(&app, &path).clone();
    let d = &mut board.free.dimensions[0];
    d.style.text_position = DimensionTextPosition::Manual;
    d.text_at = Point::new(1.5, -0.75);
    assert!(board_uses_dimension_style(&board));
    assert_eq!(snxpcb_token(&board), SNXPCB_FORMAT_V1_11);
    let (token, text) = saved_token(&board);
    assert_eq!(token, SNXPCB_FORMAT_V1_11);
    let back = SnxPcb::parse(&text).expect("reads").board;
    assert_eq!(back.free.dimensions[0].text_at, Point::new(1.5, -0.75));
    assert_eq!(back.free.dimensions, board.free.dimensions);
}
