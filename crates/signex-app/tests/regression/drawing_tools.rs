//! #1267: the Bezier and dimension tools are live: armed from the Active
//! Bar, they place one curve in four clicks and one dimension in three,
//! and both are drawings (annotation only, no connectivity).

use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{Message, Signex, Tool};
use signex_types::schematic::{DimensionDirection, SchDrawing, SchematicSheet};

use crate::paste_support::sheet::{app_with_sheet, click, empty_sheet};

fn arm(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

fn drawings(app: &Signex) -> &[SchDrawing] {
    &app.document_state
        .active_engine()
        .unwrap()
        .document()
        .drawings
}

fn sheet() -> SchematicSheet {
    empty_sheet()
}

#[test]
fn the_bezier_tool_is_no_longer_planned_and_places_in_four_clicks() {
    let mut app = app_with_sheet(sheet());
    arm(&mut app, ActiveBarAction::DrawBezier);
    assert_eq!(app.interaction_state.current_tool, Tool::Bezier);
    for at in [(10.16, 10.16), (12.7, 20.32), (25.4, 20.32), (27.94, 10.16)] {
        click(&mut app, at);
    }
    assert!(
        matches!(drawings(&app), [SchDrawing::Bezier(b)] if b.stroke_color.is_none()),
        "{:?}",
        drawings(&app)
    );
}

#[test]
fn linear_and_aligned_dimensions_place_in_three_clicks() {
    for (action, tool, direction) in [
        (
            ActiveBarAction::PlaceLinearDimension,
            Tool::LinearDimension,
            DimensionDirection::Vertical,
        ),
        (
            ActiveBarAction::PlaceAlignedDimension,
            Tool::AlignedDimension,
            DimensionDirection::Aligned,
        ),
    ] {
        let mut app = app_with_sheet(sheet());
        arm(&mut app, action);
        assert_eq!(app.interaction_state.current_tool, tool);
        for at in [(10.16, 10.16), (12.7, 30.48), (20.32, 20.32)] {
            click(&mut app, at);
        }
        let [SchDrawing::Dimension(d)] = drawings(&app) else {
            panic!("one dimension, got {:?}", drawings(&app));
        };
        assert_eq!(d.direction, direction);
        assert!(d.measured_mm() > 0.0);
    }
}

#[test]
fn the_table_tool_places_one_table_and_returns_to_select() {
    let mut app = app_with_sheet(sheet());
    arm(&mut app, ActiveBarAction::DrawTable);
    assert_eq!(app.interaction_state.current_tool, Tool::Table);
    click(&mut app, (25.4, 25.4));
    assert!(matches!(drawings(&app), [SchDrawing::Table(_)]));
    assert_eq!(app.interaction_state.current_tool, Tool::Select);
}
