//! #2868: a new dimension starts from the PCB Defaults of Preferences; the
//! Dimension tool's form overrides them for the session.

use signex_engine::pcb::primitive_edit::DimensionStyleEdit;
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
