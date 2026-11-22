//! m54 Navigator, m55 Filter and m56 SCH Filter.

use super::*;

fn navigator_labels(app: &Signex) -> Vec<String> {
    app.document_state
        .panel_ctx
        .navigator
        .visible_rows(&I18n::en_us())
        .into_iter()
        .map(|row| row.label)
        .collect()
}

fn navigator_filter(app: &mut Signex, msg: NavigatorMsg) {
    let _ = app.update(panel(PanelMsg::NavigatorPanel(msg)));
}

fn sch_filter(app: &mut Signex, msg: SchFilterMsg) {
    let _ = app.update(panel(PanelMsg::SchFilter(msg)));
}

/// Box the whole sheet and return the kinds it took.
fn box_whole_sheet(app: &mut Signex) -> Vec<SelectedKind> {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::BoxSelect {
        x1: 0.0,
        y1: 0.0,
        x2: 400.0,
        y2: 400.0,
        direction: BoxDirection::LeftToRight,
    }));
    app.interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|item| item.kind)
        .collect()
}

#[test]
fn m54_the_filter_narrows_the_sheets_and_clear_filter_brings_them_back() {
    let (mut app, _fx) = sample();
    open_panel(&mut app, PanelKind::Navigator);
    app.settle_navigator();
    let all = navigator_labels(&app);
    assert!(!all.is_empty(), "the project's sheet is listed");

    navigator_filter(
        &mut app,
        NavigatorMsg::FilterChanged("LINEAR-regulator.snx".into()),
    );
    assert_eq!(navigator_labels(&app), all, "the file name matches, any case");

    navigator_filter(
        &mut app,
        NavigatorMsg::FilterChanged("no sheet is called this".into()),
    );
    assert!(navigator_labels(&app).is_empty(), "nothing matches");

    navigator_filter(&mut app, NavigatorMsg::FilterCleared);
    assert_eq!(app.document_state.panel_ctx.navigator.filter, "");
    assert_eq!(navigator_labels(&app), all);
    assert!(is_clean(&app), "filtering is no edit");
}

#[test]
fn m54_the_focused_sheet_is_marked_and_a_click_highlights_its_row() {
    let (mut app, _fx) = sample();
    open_panel(&mut app, PanelKind::Navigator);
    app.settle_navigator();
    let rows = app
        .document_state
        .panel_ctx
        .navigator
        .visible_rows(&I18n::en_us());
    let row = rows.first().expect("the root row").clone();
    assert!(row.is_active, "the sheet in view is the marked row");
    assert_eq!(row.file, "linear-regulator.snxsch");

    drive(
        &mut app,
        panel(PanelMsg::Navigator(TreeMsg::Select(row.key.clone()))),
    );

    assert_eq!(
        app.document_state.panel_ctx.navigator.selected.as_ref(),
        Some(&row.path)
    );
    assert!(is_clean(&app));
}

#[test]
fn m54_with_no_project_it_lists_nothing_and_its_button_asks_for_one() {
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    assert!(navigator_labels(&app).is_empty());
    let task = app.update(panel(PanelMsg::NavigatorPanel(
        NavigatorMsg::OpenProjectPressed,
    )));
    let sent = TaskDriver::new().outputs(task).expect("the task finishes");
    assert!(
        sent.iter()
            .any(|m| matches!(m, Message::Menu(MenuMessage::OpenProject)))
    );
}

#[test]
fn m55_the_filter_panel_follows_a_schematic_tab_and_counts_its_wires() {
    let (mut app, _fx) = sample();
    open_panel(&mut app, PanelKind::Filter);
    let wires = doc_counts(&app).1;
    let facts = &app.document_state.panel_ctx.selection_filter;
    assert_eq!(facts.editor, FilterEditor::Schematic);
    assert_eq!(facts.schematic.get(SelectionFilter::Wires), wires);
}

#[test]
fn m55_the_filter_panel_on_a_board_edits_the_board_filter() {
    let fx = fixture::fixture().expect("fixture");
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    drive(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.pcb.clone()))),
    );
    open_panel(&mut app, PanelKind::Filter);
    assert_eq!(
        app.document_state.panel_ctx.selection_filter.editor,
        FilterEditor::Board
    );
    let tracks_on = |app: &Signex| {
        app.document_state
            .panel_ctx
            .pcb_board
            .as_ref()
            .expect("the board summary")
            .filter
            .is_on(PickKind::Tracks)
    };
    let before = tracks_on(&app);
    let schematic_kinds = on_count(&app);

    let toggle = PanelMsg::PcbBoard(PcbBoardMsg::FilterKindToggled(PickKind::Tracks));
    let _ = app.update(panel(toggle.clone()));
    assert_eq!(tracks_on(&app), !before, "the board filter changed");
    assert_eq!(on_count(&app), schematic_kinds, "the sheet filter did not");

    let _ = app.update(panel(toggle));
    assert_eq!(tracks_on(&app), before);
}

#[test]
fn m55_with_no_document_the_filter_panel_has_no_editor() {
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    open_panel(&mut app, PanelKind::Filter);
    assert_eq!(
        app.document_state.panel_ctx.selection_filter.editor,
        FilterEditor::None
    );
}

#[test]
fn m56_a_kind_turned_off_is_left_out_of_a_box_select_and_back_on_is_taken() {
    let (mut app, _fx) = sample();
    assert!(box_whole_sheet(&mut app).contains(&SelectedKind::Wire));

    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    assert!(!is_on(&app, SelectionFilter::Wires));
    assert!(
        !app.document_state
            .panel_ctx
            .selection_filters
            .contains(&SelectionFilter::Wires),
        "the panel shows it off"
    );
    assert!(!box_whole_sheet(&mut app).contains(&SelectedKind::Wire));

    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    assert!(box_whole_sheet(&mut app).contains(&SelectedKind::Wire));
    assert!(is_clean(&app), "the filter is no document edit");
}

#[test]
fn m56_none_selects_nothing_and_all_turns_every_kind_on() {
    let (mut app, _fx) = sample();
    sch_filter(&mut app, SchFilterMsg::NonePicked);
    assert_eq!(on_count(&app), 0);
    assert!(box_whole_sheet(&mut app).is_empty());

    sch_filter(&mut app, SchFilterMsg::AllPicked);
    assert_eq!(on_count(&app), SelectionFilter::ALL.len());
    assert!(!box_whole_sheet(&mut app).is_empty());
}

#[test]
fn m56_a_saved_preset_brings_its_kinds_back_when_applied() {
    let (mut app, _fx) = sample();
    sch_filter(&mut app, SchFilterMsg::NonePicked);
    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Buses));
    sch_filter(&mut app, SchFilterMsg::PresetSaved);
    assert_eq!(app.interaction_state.custom_filter_presets.len(), 1);
    assert_eq!(
        app.document_state.panel_ctx.custom_filter_presets.len(),
        1,
        "the panel lists it"
    );

    sch_filter(&mut app, SchFilterMsg::AllPicked);
    sch_filter(&mut app, SchFilterMsg::PresetApplied(0));

    assert_eq!(on_count(&app), 2);
    assert!(is_on(&app, SelectionFilter::Wires) && is_on(&app, SelectionFilter::Buses));
}

#[test]
fn m56_a_preset_past_the_end_changes_nothing() {
    let (mut app, _fx) = sample();
    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Wires));
    let on = on_count(&app);
    sch_filter(&mut app, SchFilterMsg::PresetApplied(3));
    sch_filter(&mut app, SchFilterMsg::PresetDeleted(3));
    sch_filter(&mut app, SchFilterMsg::PresetRenamed(3, "Nothing".into()));
    assert_eq!(on_count(&app), on);
    assert!(app.interaction_state.custom_filter_presets.is_empty());
}

#[test]
fn m56_save_current_stops_at_the_preset_limit() {
    let (mut app, _fx) = sample();
    for _ in 0..CUSTOM_FILTER_PRESET_LIMIT + 1 {
        sch_filter(&mut app, SchFilterMsg::PresetSaved);
    }
    assert_eq!(
        app.interaction_state.custom_filter_presets.len(),
        CUSTOM_FILTER_PRESET_LIMIT
    );
}

#[test]
fn m56_a_rename_lands_on_enter_and_typing_alone_keeps_the_old_name() {
    let (mut app, _fx) = sample();
    sch_filter(&mut app, SchFilterMsg::PresetSaved);
    let old = app.interaction_state.custom_filter_presets[0].name.clone();

    sch_filter(&mut app, SchFilterMsg::PresetRenameStarted(0));
    sch_filter(&mut app, SchFilterMsg::PresetRenameTyped("Wiring".into()));
    assert_eq!(
        app.interaction_state.custom_filter_presets[0].name, old,
        "typing is not a rename yet"
    );

    sch_filter(&mut app, SchFilterMsg::PresetRenamed(0, "Wiring".into()));
    assert_eq!(app.interaction_state.custom_filter_presets[0].name, "Wiring");
    assert!(app.interaction_state.filter_preset_rename.is_none());
}

#[test]
fn m56_delete_removes_the_preset_and_leaves_the_active_kinds() {
    let (mut app, _fx) = sample();
    sch_filter(&mut app, SchFilterMsg::PresetSaved);
    sch_filter(&mut app, SchFilterMsg::KindToggled(SelectionFilter::Texts));
    let on = on_count(&app);
    sch_filter(&mut app, SchFilterMsg::PresetDeleted(0));
    assert!(app.interaction_state.custom_filter_presets.is_empty());
    assert!(app.document_state.panel_ctx.custom_filter_presets.is_empty());
    assert_eq!(on_count(&app), on);
}
