//! m52 Components panel and m53 its row menu.

use super::*;

fn with_stock() -> (Signex, Fixture) {
    let (mut app, fx) = sample();
    fixture::mount_stock(&mut app, &fx).expect("stock mounts");
    set_filter(&mut app, "");
    (app, fx)
}

/// Type into the filter; the next finished update rebuilds the row lists.
fn set_filter(app: &mut Signex, text: &str) {
    let _ = app.update(library(LibraryMessage::ComponentsPanelSetFilter(
        text.to_string(),
    )));
    let _ = app.update(Message::Ui(UiMsg::GridPickerClose));
}

fn stock_key(app: &Signex, fx: &Fixture, pn: &str) -> ComponentsPanelRowKey {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .find(|(_, c)| c.internal_pn.as_str() == pn)
        .unwrap_or_else(|| panic!("Stock lists {pn}"));
    ComponentsPanelRowKey {
        library_path: fx.stock.clone(),
        table: app
            .library
            .table_of_row(&fx.stock, row.row_id)
            .expect("the row has a table"),
        row_id: RowId::from_uuid(row.row_id),
    }
}

fn visible_parts(app: &Signex, fx: &Fixture) -> usize {
    app.library.components_panel.rows.visible_count(&fx.stock)
}

fn right_click(app: &mut Signex, key: ComponentsPanelRowKey) {
    drive(
        app,
        library(LibraryMessage::ComponentsPanelRowRightClicked(key)),
    );
}

fn pick(app: &mut Signex, action: ComponentsRowAction) {
    drive(
        app,
        library(LibraryMessage::ComponentsPanelRowMenuPicked(action)),
    );
}

#[test]
fn m52_typing_a_filter_narrows_the_parts_and_clearing_it_brings_them_back() {
    let (mut app, fx) = with_stock();
    let all = visible_parts(&app, &fx);
    assert!(all > 1, "the stock library lists several parts: {all}");

    set_filter(&mut app, "stock-r");
    let narrowed = visible_parts(&app, &fx);
    assert!(
        (1..all).contains(&narrowed),
        "the filter keeps some parts, not all: {narrowed} of {all}"
    );

    set_filter(&mut app, "");
    assert_eq!(visible_parts(&app, &fx), all);
}

#[test]
fn m52_a_filter_nothing_matches_lists_no_parts_and_edits_nothing() {
    let (mut app, fx) = with_stock();
    let before = doc_counts(&app);
    set_filter(&mut app, "no part is called this");
    assert_eq!(visible_parts(&app, &fx), 0);
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m52_a_source_chip_is_remembered_and_all_clears_it() {
    let (mut app, _fx) = with_stock();
    let _ = app.update(library(LibraryMessage::ComponentsPanelSourceFilterChanged(
        Some(ComponentsMountSource::Stock),
    )));
    assert_eq!(
        app.library.components_panel.source_filter,
        Some(ComponentsMountSource::Stock)
    );
    let _ = app.update(library(LibraryMessage::ComponentsPanelSourceFilterChanged(
        None,
    )));
    assert_eq!(app.library.components_panel.source_filter, None);
}

#[test]
fn m52_clicking_a_part_fills_the_details_and_places_nothing() {
    let (mut app, fx) = with_stock();
    let key = stock_key(&app, &fx, "STOCK-R");
    let before = doc_counts(&app);
    let tabs = app.document_state.tabs.len();

    drive(
        &mut app,
        library(LibraryMessage::ComponentsPanelRowPressed(key.clone())),
    );
    drive(
        &mut app,
        Message::Ui(UiMsg::DragEnd(iced::window::Id::unique())),
    );

    let state = &app.library.components_panel;
    assert_eq!(state.selected.as_ref(), Some(&key));
    let loaded = state.details.loaded.as_deref().expect("details");
    assert_eq!(loaded.internal_pn, "STOCK-R");
    assert_eq!(doc_counts(&app), before, "a click places nothing");
    assert_eq!(app.document_state.tabs.len(), tabs, "a click opens no tab");
    assert!(app.interaction_state.placement.is_none());
    assert!(is_clean(&app));
}

#[test]
fn m52_the_previews_switch_changes_the_row_style_and_back() {
    let (mut app, _fx) = with_stock();
    app.library.components_panel.row_style = ComponentsRowStyle::Compact;
    let _ = app.update(library(LibraryMessage::ComponentsPanelRowStylePicked(
        ComponentsRowStyle::Previews,
    )));
    assert!(app.library.components_panel.row_style.shows_previews());
    let _ = app.update(library(LibraryMessage::ComponentsPanelRowStylePicked(
        ComponentsRowStyle::Compact,
    )));
    assert!(!app.library.components_panel.row_style.shows_previews());
}

#[test]
fn m52_the_card_preview_zooms_pans_and_a_fit_puts_it_back() {
    let (mut app, _fx) = with_stock();
    let view = |app: &Signex| {
        app.library
            .components_panel
            .preview_views
            .of(PreviewPane::Footprint)
    };
    let moved = |app: &mut Signex, gesture: PreviewGesture| {
        let _ = app.update(library(LibraryMessage::ComponentsPanelPreviewMoved(
            PreviewPane::Footprint,
            gesture,
        )));
    };
    assert_eq!(view(&app), PreviewView::default());

    moved(
        &mut app,
        PreviewGesture::WheelScrolled {
            anchor: Point::new(40.0, 40.0),
            notches: 1.0,
        },
    );
    let zoomed = view(&app);
    assert_ne!(zoomed, PreviewView::default(), "the wheel zooms");

    moved(&mut app, PreviewGesture::Dragged(Vector::new(6.0, 4.0)));
    assert_ne!(view(&app), zoomed, "a drag pans");
    let symbol = app
        .library
        .components_panel
        .preview_views
        .of(PreviewPane::Symbol);
    assert_eq!(symbol, PreviewView::default(), "the other preview stays");

    moved(&mut app, PreviewGesture::FitRequested);
    assert_eq!(view(&app), PreviewView::default(), "a double-click fits");
}

#[test]
fn m52_the_empty_panels_open_project_button_asks_for_a_project() {
    let (mut app, _fx) = sample();
    let task = app.update(library(LibraryMessage::ComponentsPanelOpenProjectClicked));
    let sent = TaskDriver::new().outputs(task).expect("the task finishes");
    assert!(
        sent.iter()
            .any(|m| matches!(m, Message::Menu(MenuMessage::OpenProject))),
        "the button runs Open Project"
    );
}

#[test]
fn m53_right_click_selects_the_row_and_place_puts_the_part_on_the_cursor() {
    let (mut app, fx) = with_stock();
    let key = stock_key(&app, &fx, "STOCK-R");
    let before = doc_counts(&app);

    right_click(&mut app, key.clone());
    let menu = app.library.components_panel.row_menu.as_ref();
    assert_eq!(menu.map(|m| &m.key), Some(&key), "the menu is on the row");
    assert_eq!(app.library.components_panel.selected.as_ref(), Some(&key));

    pick(&mut app, ComponentsRowAction::Place);
    assert!(app.library.components_panel.row_menu.is_none());
    assert!(
        app.interaction_state.placement.is_some(),
        "the part follows the cursor"
    );
    assert_eq!(doc_counts(&app), before, "nothing lands before a click");

    // A right-click ends the placement with nothing placed.
    drive(
        &mut app,
        Message::ContextMenu(ContextMenuMsg::Show(400.0, 300.0)),
    );
    assert!(app.interaction_state.placement.is_none());
    assert_eq!(doc_counts(&app), before);
    assert!(is_clean(&app));
}

#[test]
fn m53_edit_component_opens_the_row_for_editing() {
    let (mut app, fx) = with_stock();
    let key = stock_key(&app, &fx, "STOCK-R");
    assert!(app.library.editors.is_empty());
    right_click(&mut app, key);
    pick(&mut app, ComponentsRowAction::Open);
    assert_eq!(app.library.editors.len(), 1, "the row's editor opened");
    assert!(app.library.components_panel.row_menu.is_none());
}

#[test]
fn m53_dismissing_the_row_menu_places_and_opens_nothing() {
    let (mut app, fx) = with_stock();
    let key = stock_key(&app, &fx, "STOCK-R");
    let tabs = app.document_state.tabs.len();
    right_click(&mut app, key);

    let _ = app.update(library(LibraryMessage::ComponentsPanelRowMenuDismissed));

    assert!(app.library.components_panel.row_menu.is_none());
    assert!(app.interaction_state.placement.is_none());
    assert!(app.library.editors.is_empty());
    assert_eq!(app.document_state.tabs.len(), tabs);
}

#[test]
fn m53_a_pick_with_no_menu_open_does_nothing() {
    let (mut app, _fx) = with_stock();
    pick(&mut app, ComponentsRowAction::Place);
    assert!(app.interaction_state.placement.is_none());
    assert!(app.library.editors.is_empty());
}
