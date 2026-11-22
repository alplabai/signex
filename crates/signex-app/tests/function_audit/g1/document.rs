//! Mockup row 40: Document Options, with nothing selected.

use super::support::{Bench, bench, empty_sheet};
use signex_app::panels::{
    DocumentOptionsMsg, PageOrigin, PanelMsg, PaperEdge, SchematicSheetColor, SheetColor,
    SheetFrameEdit, SheetVariableMsg, TitleBlockField, paper_dimensions,
};
use signex_types::schematic::SheetOrientation;

fn doc_bench() -> Bench {
    bench(empty_sheet())
}

fn title(b: &Bench) -> Option<String> {
    b.sheet().title_block.get("title").cloned()
}

fn add_variable(b: &mut Bench, name: &str) {
    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::NewNameTyped(
        name.to_string(),
    )));
    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::Added));
}

#[test]
fn m40_picking_a_paper_size_changes_the_sheet_and_the_drawn_page() {
    let mut b = doc_bench();
    assert!(!b.dirty(), "starts clean");
    b.panel(PanelMsg::SetPaperSize("A3".into()));
    assert_eq!(b.sheet().paper_size, "A3");
    assert!(b.dirty());
    let canvas = b.app.interaction_state.active_canvas();
    assert_eq!(canvas.paper_width_mm, paper_dimensions("A3").0);
}

#[test]
fn m40_undo_of_a_paper_size_restores_the_page_and_clears_the_dirty_mark() {
    let mut b = doc_bench();
    b.panel(PanelMsg::SetPaperSize("A3".into()));
    b.undo();
    assert_eq!(b.sheet().paper_size, "A4");
    assert!(!b.dirty(), "back at the save point");
    let canvas = b.app.interaction_state.active_canvas();
    assert_eq!(canvas.paper_width_mm, paper_dimensions("A4").0);
    b.redo();
    assert_eq!(b.sheet().paper_size, "A3");
    assert!(b.dirty());
}

#[test]
fn m40_turning_the_sheet_to_portrait_changes_its_stored_size() {
    let mut b = doc_bench();
    b.panel(PanelMsg::SetSheetOrientation(SheetOrientation::Portrait));
    assert_ne!(b.sheet().paper_size, "A4");
    b.undo();
    assert_eq!(b.sheet().paper_size, "A4");
}

#[test]
fn m40_a_typed_custom_edge_applies_on_enter_and_not_before() {
    let mut b = doc_bench();
    b.panel(PanelMsg::SetPaperSize("User:200x150".into()));
    let before = b.sheet().paper_size.clone();
    b.panel(PanelMsg::CustomPaperEdgeTyping(
        PaperEdge::Width,
        "250".into(),
    ));
    assert_eq!(b.sheet().paper_size, before, "typing alone changes nothing");
    b.panel(PanelMsg::CommitCustomPaperEdge(PaperEdge::Width));
    assert!(
        b.sheet().paper_size.contains("250"),
        "{}",
        b.sheet().paper_size
    );
}

#[test]
fn m40_a_custom_edge_that_is_not_a_number_keeps_the_page() {
    let mut b = doc_bench();
    b.panel(PanelMsg::SetPaperSize("User:200x150".into()));
    let before = paper_dimensions(&b.sheet().paper_size);
    b.panel(PanelMsg::CustomPaperEdgeTyping(
        PaperEdge::Width,
        "wide".into(),
    ));
    b.panel(PanelMsg::CommitCustomPaperEdge(PaperEdge::Width));
    assert_eq!(paper_dimensions(&b.sheet().paper_size), before);
}

#[test]
fn m40_border_title_block_and_zone_counts_are_undoable_sheet_edits() {
    let mut b = doc_bench();
    let original = b.sheet().frame;
    b.panel(PanelMsg::EditSheetFrame(SheetFrameEdit::ShowBorder(false)));
    assert!(!b.sheet().frame.show_border);
    assert!(b.dirty());
    b.panel(PanelMsg::EditSheetFrame(SheetFrameEdit::ShowTitleBlock(
        false,
    )));
    assert!(!b.sheet().frame.show_title_block);
    b.panel(PanelMsg::EditSheetFrame(SheetFrameEdit::VerticalZones(6)));
    assert_eq!(b.sheet().frame.vertical_zones, 6);
    b.panel(PanelMsg::EditSheetFrame(SheetFrameEdit::HorizontalZones(8)));
    assert_eq!(b.sheet().frame.horizontal_zones, 8);
    for _ in 0..4 {
        b.undo();
    }
    assert_eq!(b.sheet().frame, original, "one undo step each");
    assert!(!b.dirty());
}

#[test]
fn m40_the_page_origin_is_an_undoable_sheet_edit() {
    let mut b = doc_bench();
    let original = b.sheet().page.origin;
    assert_eq!(original, PageOrigin::LowerLeft);
    b.panel(PanelMsg::SetPageOrigin(PageOrigin::UpperLeft));
    assert_eq!(b.sheet().page.origin, PageOrigin::UpperLeft);
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().page.origin, original);
}

#[test]
fn m40_the_sheet_colour_is_saved_with_the_sheet_and_undoes() {
    let mut b = doc_bench();
    assert_eq!(b.sheet().page.color, None);
    let preset = SchematicSheetColor::Preset(SheetColor::ALL[0]);
    b.panel(PanelMsg::SetSheetColor(preset));
    assert_eq!(b.sheet().page.color, preset.stored());
    assert!(b.dirty());
    b.undo();
    assert_eq!(b.sheet().page.color, None);
    assert!(!b.dirty());
}

#[test]
fn m40_a_title_block_value_applies_on_enter_trimmed_and_undoes() {
    let mut b = doc_bench();
    let field = TitleBlockField::Title;
    b.panel(PanelMsg::TitleBlockTyping(field, "  Buck stage ".into()));
    assert_eq!(title(&b), None, "typing alone leaves the sheet");
    assert!(!b.dirty());
    b.panel(PanelMsg::CommitTitleBlockField(field));
    assert_eq!(title(&b).as_deref(), Some("Buck stage"));
    assert!(b.dirty());
    b.undo();
    assert_eq!(title(&b), None);
    assert!(!b.dirty());
}

#[test]
fn m40_an_emptied_title_block_value_clears_the_field() {
    let mut b = doc_bench();
    let field = TitleBlockField::Title;
    b.panel(PanelMsg::TitleBlockTyping(field, "Buck stage".into()));
    b.panel(PanelMsg::CommitTitleBlockField(field));
    b.panel(PanelMsg::TitleBlockTyping(field, "   ".into()));
    b.panel(PanelMsg::CommitTitleBlockField(field));
    assert!(title(&b).is_none_or(|text| text.is_empty()));
}

#[test]
fn m40_a_sheet_variable_is_added_valued_and_removed_each_in_one_undo_step() {
    let mut b = doc_bench();
    add_variable(&mut b, "VIN");
    assert_eq!(b.sheet().variables.get("VIN").map(String::as_str), Some(""));
    assert!(b.dirty());

    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::ValueTyped {
        name: "VIN".into(),
        text: "12".into(),
    }));
    assert_eq!(
        b.sheet().variables.get("VIN").map(String::as_str),
        Some(""),
        "typing alone changes nothing"
    );
    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::ValueSubmitted(
        "VIN".into(),
    )));
    assert_eq!(b.sheet().variables["VIN"], "12");

    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::Removed(
        "VIN".into(),
    )));
    assert!(!b.sheet().variables.contains_key("VIN"));
    b.undo();
    assert_eq!(b.sheet().variables["VIN"], "12");
    b.undo();
    assert_eq!(b.sheet().variables["VIN"], "");
    b.undo();
    assert!(b.sheet().variables.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m40_a_variable_name_that_is_not_an_identifier_is_refused() {
    let mut b = doc_bench();
    add_variable(&mut b, "1 bad");
    assert!(b.sheet().variables.is_empty());
    assert!(!b.dirty());
}

#[test]
fn m40_adding_a_variable_the_sheet_already_has_keeps_its_value() {
    let mut b = doc_bench();
    add_variable(&mut b, "VIN");
    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::ValueTyped {
        name: "VIN".into(),
        text: "12".into(),
    }));
    b.panel(PanelMsg::SheetVariables(SheetVariableMsg::ValueSubmitted(
        "VIN".into(),
    )));
    add_variable(&mut b, "VIN");
    assert_eq!(b.sheet().variables["VIN"], "12");
}

#[test]
fn m40_the_grid_and_snap_boxes_change_the_view_and_leave_the_sheet_clean() {
    let mut b = doc_bench();
    let (grid, snap) = (b.app.ui_state.grid_visible, b.app.ui_state.snap_enabled);
    b.panel(PanelMsg::ToggleGrid);
    b.panel(PanelMsg::ToggleSnap);
    assert_eq!(b.app.ui_state.grid_visible, !grid);
    assert_eq!(b.app.ui_state.snap_enabled, !snap);
    b.panel(PanelMsg::SetVisibleGridSize(5.08));
    assert_eq!(b.app.ui_state.visible_grid_mm, 5.08);
    assert!(!b.dirty(), "view settings are not document edits");
}

#[test]
fn m40_the_parameters_filter_only_changes_what_the_list_shows() {
    let mut b = doc_bench();
    b.panel(PanelMsg::DocumentOptions(
        DocumentOptionsMsg::ParametersQueryChanged("rev".into()),
    ));
    assert_eq!(
        b.app
            .document_state
            .panel_ctx
            .document_options
            .parameters_query,
        "rev"
    );
    assert!(!b.dirty());
}
